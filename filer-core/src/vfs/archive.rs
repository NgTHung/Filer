//! # Archive Provider
//!
//! Provides read-only directory listing over archive files through the
//! `FsProvider` contract. The first supported backend is ZIP because existing
//! core features already depend on ZIP parsing and it supports efficient entry
//! lookup for nested paths.
//!
//! ```
//! # use std::sync::Arc;
//! # use filer_core::{ArchiveFs, FsProvider, LocalFs};
//! # use filer_core::model::registry::NodeRegistry;
//! let local = Arc::new(LocalFs::new(NodeRegistry::new()));
//! let archive = ArchiveFs::zip("bundle.zip", local);
//! assert_eq!(archive.scheme(), "archive");
//! ```

use async_trait::async_trait;
use std::collections::BTreeMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zip::ZipArchive;

use crate::errors::{CoreError, ErrorCode};
use crate::model::location::{Location, LocationDescriptor};
use crate::model::node::{FileNode, NodeEntry, NodeId, NodeKind, NodeMeta};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::{Capabilities, FsProvider};

pub struct ArchiveFs {
    archive_path: PathBuf,
    provider: Arc<dyn FsProvider>,
}

pub(crate) struct BorrowedArchiveFs<'a> {
    archive_path: PathBuf,
    provider: &'a dyn FsProvider,
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveChild {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

impl ArchiveFs {
    pub fn zip(archive_path: impl Into<PathBuf>, provider: Arc<dyn FsProvider>) -> Self {
        Self {
            archive_path: archive_path.into(),
            provider,
        }
    }

    pub(crate) async fn list_children(
        &self,
        path: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<ArchiveChild>, CoreError> {
        let archive_path = self.archive_path.clone();
        let member_path = path.to_path_buf();
        let reader = self.provider.open_reader(&archive_path, cx).await?;
        tokio::task::spawn_blocking(move || list_zip_children(reader, &member_path))
            .await
            .map_err(|e| CoreError::actor("archive_fs", e.to_string()))?
    }
}

impl<'a> BorrowedArchiveFs<'a> {
    pub(crate) fn zip(archive_path: impl Into<PathBuf>, provider: &'a dyn FsProvider) -> Self {
        Self {
            archive_path: archive_path.into(),
            provider,
        }
    }

    pub(crate) async fn list_children(
        &self,
        path: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<ArchiveChild>, CoreError> {
        let archive_path = self.archive_path.clone();
        let member_path = path.to_path_buf();
        let reader = self.provider.open_reader(&archive_path, cx).await?;
        tokio::task::spawn_blocking(move || list_zip_children(reader, &member_path))
            .await
            .map_err(|e| CoreError::actor("archive_fs", e.to_string()))?
    }
}

#[async_trait]
impl FsProvider for ArchiveFs {
    fn scheme(&self) -> &'static str {
        "archive"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        let children = self.list_children(path, cx).await?;
        Ok(children
            .into_iter()
            .map(|child| entry_for_child(&self.archive_path, child))
            .collect())
    }

    async fn read(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::unsupported_operation(format!(
            "archive member reads are not implemented yet: {}",
            path.display()
        )))
    }

    async fn read_range(
        &self,
        path: &Path,
        _start: u64,
        _len: u64,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::unsupported_operation(format!(
            "archive member range reads are not implemented yet: {}",
            path.display()
        )))
    }

    async fn exists(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        match self.list_children(path, cx).await {
            Ok(_) => Ok(true),
            Err(error) if error.code() == ErrorCode::PathNotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn metadata(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let children = self.list_children(parent, cx).await?;
        let child = children
            .into_iter()
            .find(|child| child.name == name)
            .ok_or_else(|| CoreError::not_found(path.to_path_buf()))?;
        Ok(entry_for_child(&self.archive_path, child))
    }
}

fn list_zip_children(
    reader: Box<dyn crate::vfs::provider::ReadSeek>,
    path: &Path,
) -> Result<Vec<ArchiveChild>, CoreError> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| CoreError::invalid_data(format!("Cannot open ZIP: {e}")))?;
    list_zip_directory(&mut archive, path)
}

fn list_zip_directory<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    directory: &Path,
) -> Result<Vec<ArchiveChild>, CoreError> {
    let prefix = zip_directory_prefix(directory);
    let mut children = BTreeMap::<String, ArchiveChild>::new();
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
        children
            .entry(child_name.to_string())
            .or_insert(ArchiveChild {
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

fn file_node_for_child(child: &ArchiveChild) -> FileNode {
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
    FileNode {
        id: NodeId::from_path(&child.path),
        name: child.name.clone(),
        path: child.path.clone(),
        kind,
        size: child.size,
        modified: None,
        created: None,
        accessed: None,
        meta: NodeMeta::default(),
    }
}

pub(crate) fn entry_for_child(archive_path: &Path, child: ArchiveChild) -> NodeEntry {
    let member = child.path.clone();
    let descriptor = LocationDescriptor::local(archive_path.to_path_buf()).archive_member(member);
    let display_path = descriptor.display_path();
    let node = file_node_for_child(&child);
    let navigable = child.is_dir || is_zip_path(&child.path);
    NodeEntry::from_location(Location::new(descriptor), node)
        .with_display_path(display_path)
        .with_readable(true)
        .with_navigable(navigable)
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
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
