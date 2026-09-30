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
use crate::model::node::NodeEntry;
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
            .map(|child| entry_for_segmented_child(descriptor, child))
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

fn entry_for_segmented_child(parent: &LocationDescriptor, child: ArchiveChild) -> NodeEntry {
    let descriptor = descriptor_for_child(parent, &child.path);
    crate::vfs::archive::entry_for_location(Location::new(descriptor), child)
}

fn descriptor_for_child(parent: &LocationDescriptor, child_path: &Path) -> LocationDescriptor {
    let descriptor = LocationDescriptor::local(parent.root().to_path_buf());
    let mut parent_segments = parent.segments().to_vec();
    match parent_segments.last_mut() {
        Some(LocationSegment::ArchiveMember { path }) => {
            *path = child_path.to_path_buf();
        }
        Some(LocationSegment::Virtual { .. }) | None => {
            parent_segments.push(LocationSegment::ArchiveMember {
                path: child_path.to_path_buf(),
            });
        }
    }
    descriptor.with_segments(parent_segments)
}
