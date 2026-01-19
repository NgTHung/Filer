use std::path::{Path, PathBuf};
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};

/// FUSE mount configuration
#[derive(Debug, Clone)]
pub struct FuseConfig {
    pub mount_point: PathBuf,
    pub read_only: bool,
    pub allow_other: bool,
    pub auto_unmount: bool,
}

impl Default for FuseConfig {
    fn default() -> Self {
        Self {
            mount_point: PathBuf::new(),
            read_only: false,
            allow_other: false,
            auto_unmount: true,
        }
    }
}

/// FUSE filesystem provider - exposes filer as a mountable filesystem
pub struct FuseFs {
    config: FuseConfig,
    mounted: bool,
    inner: Box<dyn FsProvider>,
}

impl FuseFs {
    pub fn new(config: FuseConfig, inner: Box<dyn FsProvider>) -> Self {
        Self {
            config,
            mounted: false,
            inner,
        }
    }

    /// Mount the filesystem
    pub async fn mount(&mut self) -> Result<(), CoreError> {
        todo!()
    }

    /// Unmount the filesystem
    pub async fn unmount(&mut self) -> Result<(), CoreError> {
        todo!()
    }

    /// Check if mounted
    pub fn is_mounted(&self) -> bool {
        self.mounted
    }

    /// FUSE: lookup
    async fn fuse_lookup(&self, parent: u64, name: &str) -> Result<FileNode, CoreError> {
        todo!()
    }

    /// FUSE: getattr
    async fn fuse_getattr(&self, ino: u64) -> Result<FileNode, CoreError> {
        todo!()
    }

    /// FUSE: readdir
    async fn fuse_readdir(&self, ino: u64, offset: i64) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    /// FUSE: read
    async fn fuse_read(&self, ino: u64, offset: i64, size: u32) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    /// FUSE: write
    async fn fuse_write(&self, ino: u64, offset: i64, data: &[u8]) -> Result<u32, CoreError> {
        todo!()
    }
}

#[async_trait]
impl FsProvider for FuseFs {
    fn scheme(&self) -> &'static str {
        "fuse"
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        self.inner.list(path).await
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError> {
        self.inner.read(path).await
    }

    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        self.inner.read_range(path, start, len).await
    }

    async fn exists(&self, path: &Path) -> bool {
        self.inner.exists(path).await
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        self.inner.metadata(path).await
    }
}
