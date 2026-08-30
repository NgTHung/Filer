//! # Location-native node entries
//!
//! This module defines the rows returned by providers and consumed by core
//! workflows. Each row keeps its reconstructable [`LocationRef`], so callers
//! do not need a second path or numeric identity table.
//!
//! ```
//! use filer_core::{Location, LocationRef, NodeEntry};
//! use filer_core::model::node::NodeKind;
//!
//! let location = Location::local("/tmp/report.txt");
//! let entry = NodeEntry::from_location_ref(
//!     LocationRef::from_location(&location),
//!     "report.txt",
//!     NodeKind::File { extension: Some("txt".to_string()) },
//! );
//! assert_eq!(entry.location.identity(), location.id());
//! ```

use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::CoreError;
use crate::model::location::{Location, LocationRef};

/// Direct-local runtime handle retained for compatibility APIs.
///
/// New provider, pipeline, cache, scanner, and search code must use
/// [`LocationRef`] through [`NodeEntry`]. API-008 owns deleting this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Location-native node row for provider, directory, and search results.
///
/// The location is the only row identity. `display_path` is optional
/// presentation data used when a provider needs a human-readable route that
/// differs from the descriptor's default display.
#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub location: LocationRef,
    pub display_path: Option<String>,
    pub capabilities: NodeEntryCapabilities,
    pub name: String,
    pub kind: NodeKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub meta: NodeMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeEntryCapabilities {
    pub read: bool,
    pub navigate: bool,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    File { extension: Option<String> },
    Directory { children_count: Option<u32> },
    Symlink { target: PathBuf },
}

#[derive(Debug, Clone, Default)]
pub struct NodeMeta {
    pub hidden: bool,
    pub readonly: bool,
    pub permissions: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

impl NodeEntry {
    /// Create a fully populated local entry from a path.
    pub fn from_path(path: PathBuf) -> Result<Self, CoreError> {
        use std::fs;

        let expanded_path = expand_path(path)?;
        let metadata = fs::metadata(&expanded_path)
            .map_err(|e| CoreError::from_io_error(e, expanded_path.clone()))?;
        Self::from_metadata(metadata, expanded_path)
    }

    /// Create a local entry from already-fetched metadata.
    pub fn from_metadata(meta: Metadata, path: PathBuf) -> Result<Self, CoreError> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        Ok(Self::from_parts(
            LocationRef::from_location(&Location::local(path.clone())),
            name.as_str(),
            kind_from_metadata(&meta, &path),
            meta.len(),
            meta.modified().ok(),
            meta.created().ok(),
            meta.accessed().ok(),
            meta_for_path(&meta, &name),
        ))
    }

    /// Create a local entry from directory-entry type data without a stat.
    pub fn from_dir_entry(path: PathBuf, file_type: std::fs::FileType) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        Self::from_parts(
            LocationRef::from_location(&Location::local(path.clone())),
            name.as_str(),
            kind_from_file_type(&file_type, &path),
            0,
            None,
            None,
            None,
            NodeMeta {
                hidden: is_hidden_name(&name),
                ..NodeMeta::default()
            },
        )
    }

    /// Create a provider-owned entry with default metadata.
    pub fn from_location_ref(
        location: LocationRef,
        name: impl Into<String>,
        kind: NodeKind,
    ) -> Self {
        let name = name.into();
        let navigate = matches!(kind, NodeKind::Directory { .. });
        Self::from_parts(
            location,
            &name,
            kind,
            0,
            None,
            None,
            None,
            NodeMeta::default(),
        )
        .with_readable(true)
        .with_navigable(navigate)
    }

    fn from_parts(
        location: LocationRef,
        name: &str,
        kind: NodeKind,
        size: u64,
        modified: Option<SystemTime>,
        created: Option<SystemTime>,
        accessed: Option<SystemTime>,
        meta: NodeMeta,
    ) -> Self {
        let navigate = matches!(kind, NodeKind::Directory { .. });
        Self {
            location,
            display_path: None,
            capabilities: NodeEntryCapabilities {
                read: true,
                navigate,
            },
            name: name.to_string(),
            kind,
            size,
            modified,
            created,
            accessed,
            meta,
        }
    }

    pub fn with_display_path(mut self, display_path: impl Into<String>) -> Self {
        self.display_path = Some(display_path.into());
        self
    }

    pub fn with_readable(mut self, readable: bool) -> Self {
        self.capabilities.read = readable;
        self
    }

    pub fn with_navigable(mut self, navigable: bool) -> Self {
        self.capabilities.navigate = navigable;
        self
    }

    pub(crate) fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Directory { .. })
    }

    pub fn is_file(&self) -> bool {
        matches!(self.kind, NodeKind::File { .. })
    }

    pub fn extension(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::File { extension } => extension.as_deref(),
            _ => None,
        }
    }

    /// Human-readable size string, for example `1.5 MB`.
    pub fn size_formatted(&self) -> String {
        crate::utils::size::format_size(self.size)
    }

    /// Resolve owner and group names through the platform's NSS database.
    #[cfg(unix)]
    pub fn load_owner_info(&mut self) -> Result<(), CoreError> {
        use std::os::unix::fs::MetadataExt;
        use users::{get_group_by_gid, get_user_by_uid};

        let path = self
            .location
            .descriptor()
            .and_then(|descriptor| descriptor.as_local_path())
            .ok_or_else(|| CoreError::invalid_input("owner information requires a local path"))?;
        let metadata =
            std::fs::metadata(path).map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        self.meta.owner =
            get_user_by_uid(metadata.uid()).map(|user| user.name().to_string_lossy().into_owned());
        self.meta.group = get_group_by_gid(metadata.gid())
            .map(|group| group.name().to_string_lossy().into_owned());
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn load_owner_info(&mut self) -> Result<(), CoreError> {
        Ok(())
    }
}

fn expand_path(path: PathBuf) -> Result<PathBuf, CoreError> {
    if path.starts_with("~")
        && let Ok(home) = std::env::var("HOME")
    {
        return Ok(PathBuf::from(home).join(
            path.strip_prefix("~")
                .map_err(|_| CoreError::invalid_input("invalid home-relative path"))?,
        ));
    }
    path.canonicalize()
        .map_err(|e| CoreError::from_io_error(e, path))
}

fn kind_from_metadata(meta: &Metadata, path: &Path) -> NodeKind {
    if meta.is_dir() {
        NodeKind::Directory {
            children_count: None,
        }
    } else if meta.is_symlink() {
        NodeKind::Symlink {
            target: std::fs::read_link(path).unwrap_or_default(),
        }
    } else {
        NodeKind::File {
            extension: path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_string),
        }
    }
}

fn kind_from_file_type(file_type: &std::fs::FileType, path: &Path) -> NodeKind {
    if file_type.is_dir() {
        NodeKind::Directory {
            children_count: None,
        }
    } else if file_type.is_symlink() {
        NodeKind::Symlink {
            target: std::fs::read_link(path).unwrap_or_default(),
        }
    } else {
        NodeKind::File {
            extension: path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_string),
        }
    }
}

fn meta_for_path(meta: &Metadata, name: &str) -> NodeMeta {
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode())
    };

    #[cfg(windows)]
    let permissions = None;

    #[cfg(unix)]
    let hidden = name.starts_with('.');

    #[cfg(windows)]
    let hidden = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
        meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
    };

    NodeMeta {
        hidden,
        readonly: meta.permissions().readonly(),
        permissions,
        owner: None,
        group: None,
    }
}

#[cfg(unix)]
fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

#[cfg(not(unix))]
fn is_hidden_name(_name: &str) -> bool {
    false
}

impl NodeId {
    /// Generate ID from path
    pub fn from_path(path: &Path) -> Self {
        NodeId({
            use rapidhash::fast::RapidHasher;
            use std::hash::Hasher;
            let mut h = RapidHasher::default();
            h.write(path.as_os_str().as_encoded_bytes());
            h.finish()
        })
    }
}
