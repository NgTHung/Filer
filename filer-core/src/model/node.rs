use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::CoreError;
use crate::model::location::{Location, LocationRef};
use crate::model::registry::NodeRegistry;

/// Direct-local runtime handle for a file node.
///
/// `NodeId` is a lightweight compatibility/cache handle derived from a path.
/// It is valid for direct-local compatibility APIs, selection state, cache
/// lookup, and registry bridging. New provider-aware transport should prefer
/// [`LocationRef`], because id-only `NodeId` values cannot reconstruct
/// segmented, remote, profile, or archive locations.
///
/// Use [`NodeRegistry`] to resolve `NodeId` to `PathBuf` when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Direct-local/provider compatibility row.
///
/// `FileNode` remains the path-era row shape used by legacy listing and search
/// events and by providers that still execute against `PathBuf`. Do not extend
/// it with canonical provider identity semantics; Location-native public
/// result APIs should convert to [`NodeEntry`].
#[derive(Debug, Clone)]
pub struct FileNode {
    pub id: NodeId,
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub meta: NodeMeta,
}

/// Location-native node row for public directory/search result APIs.
///
/// `NodeEntry` is the preferred public row shape for Location-native listing
/// and search events. The `id` field is a compatibility handle for direct-local
/// caches and selection, while `location` is the transport identity.
#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub id: NodeId,
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

impl FileNode {
    /// Create a new file node from path
    pub fn from_path(path: PathBuf, reg: Option<NodeRegistry>) -> Result<Self, CoreError> {
        use std::fs;

        let expanded_path = if path.starts_with("~") {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(path.strip_prefix("~").unwrap())
            } else {
                path.clone()
                    .canonicalize()
                    .map_err(|e| CoreError::from_io_error(e, path))?
            }
        } else {
            path.clone()
                .canonicalize()
                .map_err(|e| CoreError::from_io_error(e, path))?
        };

        // Get metadata
        let metadata = fs::metadata(&expanded_path)
            .map_err(|e| CoreError::from_io_error(e, expanded_path.clone()))?;

        // Extract file name
        let name = expanded_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Generate ID
        let id = match reg {
            Some(r) => r.register(expanded_path.clone()),
            None => NodeId::from_path(&expanded_path),
        };

        // Determine kind
        let kind = if metadata.is_dir() {
            NodeKind::Directory {
                children_count: None,
            }
        } else if metadata.is_symlink() {
            let target = fs::read_link(&expanded_path).unwrap_or_else(|_| PathBuf::new());
            NodeKind::Symlink { target }
        } else {
            let extension = expanded_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            NodeKind::File { extension }
        };

        // Get times
        let modified = metadata.modified().ok();
        let created = metadata.created().ok();
        let accessed = metadata.accessed().ok();

        // Get size
        let size = metadata.len();

        // Determine if hidden (Unix: starts with dot)
        #[cfg(unix)]
        let hidden = name.starts_with('.');
        #[cfg(windows)]
        let hidden = {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x00000002;
            metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
        };

        // Get permissions
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode())
        };

        #[cfg(windows)]
        let permissions = None;

        let readonly = metadata.permissions().readonly();

        Ok(FileNode {
            id,
            name,
            path: expanded_path,
            kind,
            size,
            modified,
            created,
            accessed,
            meta: NodeMeta {
                hidden,
                readonly,
                permissions,
                owner: None,
                group: None,
            },
        })
    }

    pub fn from_metadata(
        meta: Metadata,
        path: PathBuf,
        reg: Option<NodeRegistry>,
    ) -> Result<Self, CoreError> {
        use std::fs;
        // Extract file name
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Generate ID
        let id = match reg {
            Some(r) => r.register(path.clone()),
            None => NodeId::from_path(&path),
        };

        // Determine kind
        let kind = if meta.is_dir() {
            NodeKind::Directory {
                children_count: None,
            }
        } else if meta.is_symlink() {
            let target = fs::read_link(&path).unwrap_or_else(|_| PathBuf::new());
            NodeKind::Symlink { target }
        } else {
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            NodeKind::File { extension }
        };

        // Get times
        let modified = meta.modified().ok();
        let created = meta.created().ok();
        let accessed = meta.accessed().ok();

        // Get size
        let size = meta.len();

        // Determine if hidden (Unix: starts with dot)
        #[cfg(unix)]
        let hidden = name.starts_with('.');
        #[cfg(windows)]
        let hidden = {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x00000002;
            meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
        };

        // Get permissions
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(meta.permissions().mode())
        };

        #[cfg(windows)]
        let permissions = None;

        let readonly = meta.permissions().readonly();

        Ok(FileNode {
            id,
            name,
            path,
            kind,
            size,
            modified,
            created,
            accessed,
            meta: NodeMeta {
                hidden,
                readonly,
                permissions,
                owner: None,
                group: None,
            },
        })
    }

    /// Create a FileNode from a directory entry's file type — no stat syscall.
    ///
    /// `size`, timestamps, and `NodeMeta` fields are left at their zero/default
    /// values. Call `metadata()` on the provider when full stat info is needed.
    pub fn from_dir_entry(
        path: PathBuf,
        file_type: std::fs::FileType,
        reg: Option<NodeRegistry>,
    ) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let id = match reg {
            Some(r) => r.register(path.clone()),
            None => NodeId::from_path(&path),
        };

        let kind = if file_type.is_dir() {
            NodeKind::Directory {
                children_count: None,
            }
        } else if file_type.is_symlink() {
            let target = std::fs::read_link(&path).unwrap_or_default();
            NodeKind::Symlink { target }
        } else {
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_string);
            NodeKind::File { extension }
        };

        #[cfg(unix)]
        let hidden = name.starts_with('.');
        #[cfg(not(unix))]
        let hidden = false;

        FileNode {
            id,
            name,
            path,
            kind,
            size: 0,
            modified: None,
            created: None,
            accessed: None,
            meta: NodeMeta {
                hidden,
                ..NodeMeta::default()
            },
        }
    }

    /// Check if this is a directory
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Directory { .. })
    }

    /// Check if this is a file
    pub fn is_file(&self) -> bool {
        matches!(self.kind, NodeKind::File { .. })
    }

    /// Get file extension if any
    pub fn extension(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::File { extension } => extension.as_deref(),
            _ => None,
        }
    }

    /// Human-readable size string (e.g. "1.5 MB")
    pub fn size_formatted(&self) -> String {
        crate::utils::size::format_size(self.size)
    }

    /// Resolve owner and group names via NSS and store them in `meta`.
    ///
    /// Uses `getpwuid_r` / `getgrgid_r` under the hood, so it works with
    /// LDAP, NIS, sssd, and any other NSS-backed directory — not just local
    /// `/etc/passwd` entries. On non-Unix platforms this is a no-op.
    #[cfg(unix)]
    pub fn load_owner_info(&mut self) -> Result<(), CoreError> {
        use std::os::unix::fs::MetadataExt;
        use users::{get_group_by_gid, get_user_by_uid};

        let metadata = std::fs::metadata(&self.path)
            .map_err(|e| CoreError::from_io_error(e, self.path.clone()))?;

        self.meta.owner =
            get_user_by_uid(metadata.uid()).map(|u| u.name().to_string_lossy().into_owned());
        self.meta.group =
            get_group_by_gid(metadata.gid()).map(|g| g.name().to_string_lossy().into_owned());

        Ok(())
    }

    #[cfg(not(unix))]
    pub fn load_owner_info(&mut self) -> Result<(), CoreError> {
        Ok(())
    }
}

impl NodeEntry {
    pub fn from_file_node(node: FileNode, reg: &NodeRegistry) -> Self {
        let location = reg.location_for_path(node.path.clone());
        Self::from_file_node_with_location(node, LocationRef::from_location(&location))
    }

    pub fn from_file_node_with_location(node: FileNode, location: LocationRef) -> Self {
        Self {
            id: node.id,
            location,
            display_path: None,
            capabilities: NodeEntryCapabilities {
                read: true,
                navigate: node.is_dir(),
            },
            name: node.name,
            kind: node.kind,
            size: node.size,
            modified: node.modified,
            created: node.created,
            accessed: node.accessed,
            meta: node.meta,
        }
    }

    pub fn from_location(location: Location, node: FileNode) -> Self {
        Self::from_file_node_with_location(node, LocationRef::from_location(&location))
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

    pub(crate) fn to_file_node(&self) -> FileNode {
        FileNode {
            id: self.id,
            name: self.name.clone(),
            path: self
                .display_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(self.name.clone())),
            kind: self.kind.clone(),
            size: self.size,
            modified: self.modified,
            created: self.created,
            accessed: self.accessed,
            meta: self.meta.clone(),
        }
    }
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
