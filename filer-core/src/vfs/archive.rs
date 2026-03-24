use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::provider::{Capabilities, FsProvider};

/// Archive filesystem provider (ZIP, TAR, etc.)
pub struct ArchiveFs {
    archive_path: std::path::PathBuf,
}

impl ArchiveFs {
    pub fn new(archive_path: std::path::PathBuf) -> Self {
        Self { archive_path }
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

    async fn list(&self, _path: &Path) -> Result<Vec<FileNode>, CoreError> {
        todo!()
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    async fn read_range(&self, _path: &Path, _start: u64, _len: u64) -> Result<Vec<u8>, CoreError> {
        todo!()
    }

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> {
        todo!()
    }

    async fn metadata(&self, _path: &Path) -> Result<FileNode, CoreError> {
        todo!()
    }
}
