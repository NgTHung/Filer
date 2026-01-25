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
        let dp =
            std::fs::read_dir(path).map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let res = dp
            .filter_map(|de| {
                de.ok()
                    .and_then(|f| FileNode::from_path(f.path(), Some(self.reg.clone())).ok())
            })
            .collect::<Vec<FileNode>>();
        Ok(res)
    }
    #[cfg(target_os = "windows")]
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let dp =
            std::fs::read_dir(path).map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let res = dp
            .filter_map(|de| {
                let f = de.ok()?;
                let filename = f.path();
                let filemeta = f
                    .metadata()
                    .map_err(|e| CoreError::from_io_error(e, filename.clone()))
                    .ok()?;
                FileNode::from_metadata(filemeta, filename, Some(self.reg.clone())).ok()
            })
            .collect::<Vec<FileNode>>();
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
