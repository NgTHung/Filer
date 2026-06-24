//! # Segmented Location Resolver
//!
//! This module executes local archive-backed `Location` segments without
//! teaching every provider about nested address syntax.
//!
//! The resolver keeps provider access at the root and resolves archive members
//! in order. ZIP is the first supported archive because it is already part of
//! the default metadata feature set and supports efficient entry lookup.
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

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::errors::{CoreError, ErrorCode};
use crate::model::location::{Location, LocationDescriptor, LocationSegment, ProviderRef};
use crate::model::node::{FileNode, NodeEntry, NodeId, NodeKind, NodeMeta};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

pub struct SegmentedLocationResolver<'a> {
    provider: &'a dyn FsProvider,
}

#[derive(Debug, Clone)]
struct ZipChild {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
}

impl<'a> SegmentedLocationResolver<'a> {
    pub fn new(provider: &'a dyn FsProvider) -> Self {
        Self { provider }
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

        let root = descriptor.root().to_path_buf();
        let segments = archive_segments(descriptor.segments())?;
        let reader = self.provider.open_reader(&root, cx).await?;
        let children = tokio::task::spawn_blocking(move || list_zip_segments(reader, segments))
            .await
            .map_err(|e| CoreError::actor("segmented_resolver", e.to_string()))??;

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

fn list_zip_segments(
    reader: Box<dyn crate::vfs::provider::ReadSeek>,
    segments: Vec<PathBuf>,
) -> Result<Vec<ZipChild>, CoreError> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| CoreError::invalid_data(format!("Cannot open ZIP: {e}")))?;
    list_zip_archive(&mut archive, &segments)
}

fn list_zip_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    segments: &[PathBuf],
) -> Result<Vec<ZipChild>, CoreError> {
    let Some((first, rest)) = segments.split_first() else {
        return list_zip_directory(archive, Path::new(""));
    };

    if first.as_os_str().is_empty() {
        if rest.is_empty() {
            return list_zip_directory(archive, Path::new(""));
        }
        return Err(CoreError::invalid_location(
            ErrorCode::LocationSegmentedUnsupported,
            None,
            "empty archive segment can only target the current archive root",
        ));
    }

    if rest.is_empty() {
        return list_zip_directory(archive, first);
    }

    let inner_name = zip_entry_name(first);
    let mut inner = archive
        .by_name(&inner_name)
        .map_err(|_| CoreError::not_found(first.clone()))?;
    if inner.is_dir() {
        return Err(CoreError::invalid_location(
            ErrorCode::LocationSegmentedUnsupported,
            None,
            format!(
                "archive directory is not a nested archive: {}",
                first.display()
            ),
        ));
    }

    let mut bytes = Vec::with_capacity(inner.size() as usize);
    inner
        .read_to_end(&mut bytes)
        .map_err(|e| CoreError::invalid_data(format!("Cannot read ZIP entry: {e}")))?;
    let cursor = Cursor::new(bytes);
    let mut nested = ZipArchive::new(cursor)
        .map_err(|e| CoreError::invalid_data(format!("Cannot open nested ZIP: {e}")))?;
    list_zip_archive(&mut nested, rest)
}

fn list_zip_directory<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    directory: &Path,
) -> Result<Vec<ZipChild>, CoreError> {
    let prefix = zip_directory_prefix(directory);
    let mut children = BTreeMap::<String, ZipChild>::new();
    let mut saw_exact_file = false;

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| CoreError::invalid_data(format!("ZIP entry {index}: {e}")))?;
        let name = entry.name();
        if !prefix.is_empty() && name == prefix.trim_end_matches('/') && !entry.is_dir() {
            saw_exact_file = true;
            continue;
        }

        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let child_name = rest.split('/').next().unwrap_or("");
        if child_name.is_empty() {
            continue;
        }

        let child_path = if directory.as_os_str().is_empty() {
            PathBuf::from(child_name)
        } else {
            directory.join(child_name)
        };
        let is_dir = rest.contains('/') || entry.is_dir();
        let size = if is_dir { 0 } else { entry.size() };
        children.entry(child_name.to_string()).or_insert(ZipChild {
            name: child_name.to_string(),
            path: child_path,
            is_dir,
            size,
        });
    }

    if children.is_empty() {
        if saw_exact_file {
            return Err(CoreError::invalid_location(
                ErrorCode::LocationSegmentedUnsupported,
                None,
                format!("archive member is not a directory: {}", directory.display()),
            ));
        }
        return Err(CoreError::not_found(directory.to_path_buf()));
    }

    Ok(children.into_values().collect())
}

fn entry_for_child(parent: &LocationDescriptor, child: ZipChild) -> NodeEntry {
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

fn zip_directory_prefix(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        String::new()
    } else {
        let mut prefix = zip_entry_name(path);
        if !prefix.ends_with('/') {
            prefix.push('/');
        }
        prefix
    }
}

fn zip_entry_name(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}
