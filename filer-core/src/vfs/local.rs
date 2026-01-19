use std::path::Path;
use async_trait::async_trait;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};

/// Local filesystem provider
pub struct LocalFs;

impl LocalFs {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FsProvider for LocalFs {
    fn scheme(&self) -> &'static str {
        "file"
    }
    
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            watch: true,
            search: false,
        }
    }
    
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }
    
    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError> {
        todo!()
    }
    
    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        todo!()
    }
    
    async fn exists(&self, path: &Path) -> bool {
        todo!()
    }
    
    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        todo!()
    }
}