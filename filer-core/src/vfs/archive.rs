use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::{Capabilities, FsProvider};

/// Archive filesystem provider (ZIP, TAR, etc.)
#[allow(dead_code)]
pub struct ArchiveFs {
    archive_path: std::path::PathBuf,
}

#[allow(dead_code)]
impl ArchiveFs {
    pub fn new(archive_path: std::path::PathBuf) -> Self {
        Self { archive_path }
    }

    fn unsupported(&self) -> CoreError {
        CoreError::unsupported_operation(format!(
            "archive filesystem navigation is not implemented for {}",
            self.archive_path.display()
        ))
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

    async fn list(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<FileNode>, CoreError> {
        Err(self.unsupported())
    }

    async fn read(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Err(self.unsupported())
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Err(self.unsupported())
    }

    async fn exists(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        Err(self.unsupported())
    }

    async fn metadata(&self, _path: &Path, _cx: &ProviderCx<'_>) -> Result<FileNode, CoreError> {
        Err(self.unsupported())
    }
}
