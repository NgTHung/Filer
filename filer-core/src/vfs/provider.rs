use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;

/// Capabilities of a filesystem provider
#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub read: bool,
    pub write: bool,
    pub watch: bool,
    pub search: bool,
}

/// Trait for filesystem backends
#[async_trait]
pub trait FsProvider: Send + Sync {
    /// Unique scheme for this provider (e.g., "file", "zip", "sftp")
    fn scheme(&self) -> &'static str;
    
    /// Provider capabilities
    fn capabilities(&self) -> Capabilities;
    
    /// List contents of a directory
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError>;
    
    /// Read file contents
    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError>;
    
    /// Read partial file contents
    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError>;
    
    /// Check if path exists
    async fn exists(&self, path: &Path) -> bool;
    
    /// Get metadata for a path
    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError>;
}