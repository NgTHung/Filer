//! # Segmented Location Resolver
//!
//! This module executes local archive-backed `Location` segments without
//! teaching every provider about nested address syntax.
//!
//! The resolver keeps provider access at the root and delegates archive member
//! listing to [`ArchiveFs`]. Nested archive files need member reads, so they
//! return structured unsupported errors until archive reads land.
//!
//! ```
//! # use filer_core::{LocationDescriptor, ProviderCx};
//! # async fn example(provider: &dyn filer_core::FsProvider) -> Result<(), filer_core::CoreError> {
//! let location = LocationDescriptor::local("bundle.zip").archive_member("");
//! let entries = filer_core::SegmentedLocationResolver::new(provider)
//!     .list(&location, &ProviderCx::none())
//!     .await?;
//! # let _ = entries;
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::errors::{CoreError, ErrorCode};
use crate::model::location::{Location, LocationDescriptor, LocationSegment, ProviderRef};
use crate::model::node::{FileNode, NodeEntry, NodeId, NodeKind, NodeMeta};
use crate::vfs::archive::{ArchiveChild, ArchiveFs, BorrowedArchiveFs};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

pub struct SegmentedLocationResolver<'a> {
    provider: SegmentedProvider<'a>,
}

enum SegmentedProvider<'a> {
    Borrowed(&'a dyn FsProvider),
    Shared(Arc<dyn FsProvider>),
}

impl<'a> SegmentedLocationResolver<'a> {
    pub fn new(provider: &'a dyn FsProvider) -> Self {
        Self {
            provider: SegmentedProvider::Borrowed(provider),
        }
    }

    pub fn from_arc(provider: Arc<dyn FsProvider>) -> Self {
        Self {
            provider: SegmentedProvider::Shared(provider),
        }
    }

    pub async fn list(
        &self,
        descriptor: &LocationDescriptor,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<NodeEntry>, CoreError> {
        if descriptor.provider() != &ProviderRef::Local || descriptor.scheme() != "file" {
            return Err(CoreError::unsupported_provider(
                format!("{}:{:?}", descriptor.scheme(), descriptor.provider()),
                format!(
                    "unsupported provider route: {} {:?} {}",
                    descriptor.scheme(),
                    descriptor.provider(),
                    descriptor.root().display()
                ),
            ));
        }
        if descriptor.segments().is_empty() {
            return Err(CoreError::invalid_location(
                ErrorCode::LocationSegmentedUnsupported,
                None,
                "segmented resolver requires at least one segment",
            ));
        }

        let segments = archive_segments(descriptor.segments())?;
        let member = target_archive_member(&segments)?;
        let children = match &self.provider {
            SegmentedProvider::Borrowed(provider) => {
                BorrowedArchiveFs::zip(descriptor.root().to_path_buf(), *provider)
                    .list_children(member, cx)
                    .await?
            }
            SegmentedProvider::Shared(provider) => {
                ArchiveFs::zip(descriptor.root().to_path_buf(), provider.clone())
                    .list_children(member, cx)
                    .await?
            }
        };

        Ok(children
            .into_iter()
            .map(|child| entry_for_child(descriptor, child))
            .collect())
    }
}

fn archive_segments(segments: &[LocationSegment]) -> Result<Vec<PathBuf>, CoreError> {
    segments
        .iter()
        .map(|segment| match segment {
            LocationSegment::ArchiveMember { path } => Ok(path.clone()),
            LocationSegment::Virtual { scheme, path } => Err(CoreError::invalid_location(
                ErrorCode::LocationSegmentedUnsupported,
                None,
                format!(
                    "virtual segment is not routable yet: {scheme}:{}",
                    path.display()
                ),
            )),
        })
        .collect()
}

fn target_archive_member(segments: &[PathBuf]) -> Result<&Path, CoreError> {
    if segments.len() > 1 {
        return Err(CoreError::invalid_location(
            ErrorCode::LocationSegmentedUnsupported,
            None,
            "nested archive segments require archive member reads",
        ));
    }

    Ok(segments
        .first()
        .map(PathBuf::as_path)
        .unwrap_or_else(|| Path::new("")))
}

fn entry_for_child(parent: &LocationDescriptor, child: ArchiveChild) -> NodeEntry {
    let descriptor = descriptor_for_child(parent, &child.path);
    let location = Location::new(descriptor);
    let display_path = location.descriptor().display_path();
    let extension = child
        .path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_string);
    let kind = if child.is_dir {
        NodeKind::Directory {
            children_count: None,
        }
    } else {
        NodeKind::File { extension }
    };
    let node = FileNode {
        id: NodeId::from_path(Path::new(&display_path)),
        name: child.name,
        path: PathBuf::from(&display_path),
        kind,
        size: child.size,
        modified: None,
        created: None,
        accessed: None,
        meta: NodeMeta::default(),
    };
    let navigable = child.is_dir || is_zip_path(&child.path);
    NodeEntry::from_location(location, node)
        .with_display_path(display_path)
        .with_readable(true)
        .with_navigable(navigable)
}

fn descriptor_for_child(parent: &LocationDescriptor, child_path: &Path) -> LocationDescriptor {
    let mut descriptor = LocationDescriptor::local(parent.root().to_path_buf());
    let parent_segments: Vec<_> = parent
        .segments()
        .iter()
        .filter_map(|segment| match segment {
            LocationSegment::ArchiveMember { path } if !path.as_os_str().is_empty() => {
                Some(LocationSegment::ArchiveMember { path: path.clone() })
            }
            LocationSegment::ArchiveMember { .. } | LocationSegment::Virtual { .. } => None,
        })
        .collect();
    descriptor = descriptor.with_segments(parent_segments);
    descriptor.archive_member(child_path.to_path_buf())
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}
