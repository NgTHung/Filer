use async_trait::async_trait;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::{Capabilities, FsProvider};

/// Local filesystem provider
pub struct LocalFs {
    reg: NodeRegistry,
}

impl LocalFs {
    pub fn new(register: NodeRegistry) -> Self {
        Self { reg: register }
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

    #[cfg(target_os = "linux")]
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let mut dir =
            tokio::fs::read_dir(path).await.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir.next_entry().await.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))? {
            match FileNode::from_path(entry.path(), Some(self.reg.clone())) {
                Ok(node) => res.push(node),
                Err(e) => {
                    tracing::debug!(path = %entry.path().display(), error = %e, "skipping entry in listing");
                }
            }
        }
        Ok(res)
    }
    #[cfg(target_os = "windows")]
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let mut dir =
            tokio::fs::read_dir(path).await.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir.next_entry().await.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))? {
            let filename = entry.path();
            let filemeta = match entry.metadata().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::debug!(path = %filename.display(), error = %e, "skipping entry metadata");
                    continue;
                }
            };
            match FileNode::from_metadata(filemeta, filename.clone(), Some(self.reg.clone())) {
                Ok(node) => res.push(node),
                Err(e) => {
                    tracing::debug!(path = %filename.display(), error = %e, "skipping entry in listing");
                }
            }
        }
        Ok(res)
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, CoreError> {
        let mut f = File::open(path)
            .await
            .map_err(|err| CoreError::from_io_error(err, path.to_path_buf()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .await
            .map_err(|err| CoreError::from_io_error(err, path.to_path_buf()))?;
        Ok(buf)
    }

    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        let mut f = File::open(path)
            .await
            .map_err(|err| CoreError::from_io_error(err, path.to_path_buf()))?;
        let mut buf = vec![0; len as usize];
        f.seek(std::io::SeekFrom::Start(start)).await.map_err(|err| CoreError::from_io_error(err, path.to_path_buf()))?;
        let size = f.read(&mut buf).await.map_err(|err| CoreError::from_io_error(err, path.to_path_buf()))?;
        if size != (len as usize) {
            buf.resize(size, 0);
        }
        Ok(buf)
    }

    async fn exists(&self, path: &Path) -> Result<bool,CoreError> {
        tokio::fs::try_exists(path).await.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        FileNode::from_path(path.to_path_buf(), Some(self.reg.clone()))
    }
}
