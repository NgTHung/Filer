use std::fs::Metadata;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::CoreError;
use crate::model::registry::NodeRegistry;

/// Unique identifier for a file node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// Represents a file or directory
#[derive(Debug, Clone)]
pub struct FileNode {
    pub id: NodeId,
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub meta: NodeMeta,
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
}

impl FileNode {
    /// Create a new file node from path
    pub fn from_path(path: PathBuf, reg: Option<NodeRegistry>) -> Result<Self, CoreError> {
        use std::fs;

        // Expand tilde if present
        let expanded_path = path
            .canonicalize()
            .map_err(|e| CoreError::from_io_error(e, path))?;

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
            Some(mut r) => r.register(expanded_path.clone()),
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

        // Get size
        let size = metadata.len();

        // Determine if hidden (Unix: starts with dot)
        let hidden = name.starts_with('.');

        // Get permissions
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode())
        };

        #[cfg(not(unix))]
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
            meta: NodeMeta {
                hidden,
                readonly,
                permissions,
            },
        })
    }

    pub fn from_metadata(
        meta: Metadata,
        path: PathBuf,
        reg: Option<NodeRegistry>,
    ) -> Result<Self, CoreError> {
        use std::fs;
        let path = path
            .canonicalize()
            .map_err(|e| CoreError::from_io_error(e, path))?;
        // Extract file name
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Generate ID
        let id = match reg {
            Some(mut r) => r.register(path.clone()),
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

        // Get size
        let size = meta.len();

        // Determine if hidden (Unix: starts with dot)
        let hidden = name.starts_with('.');

        // Get permissions
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(meta.permissions().mode())
        };

        #[cfg(not(unix))]
        let permissions = None;

        let readonly = meta.permissions().readonly();

        Ok(FileNode {
            id,
            name,
            path: path,
            kind,
            size,
            modified,
            created,
            meta: NodeMeta {
                hidden,
                readonly,
                permissions,
            },
        })
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
}

impl NodeId {
    /// Generate ID from path
    pub fn from_path(path: &PathBuf) -> Self {
        NodeId(twox_hash::XxHash3_64::oneshot(
            path.to_str().unwrap().as_bytes(),
        ))
    }
}
