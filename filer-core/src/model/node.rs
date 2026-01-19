use std::path::PathBuf;
use std::time::SystemTime;

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
    pub fn from_path(path: PathBuf) -> Self {
        todo!()
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
        todo!()
    }
}