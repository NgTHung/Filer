use async_trait::async_trait;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::{Capabilities, FsProvider, ReadSeek};

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

    #[cfg(unix)]
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let mut dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?
        {
            match FileNode::from_path(entry.path(), Some(self.reg.clone())) {
                Ok(node) => res.push(node),
                Err(e) => {
                    tracing::debug!(path = %entry.path().display(), error = %e, "skipping entry in listing");
                }
            }
        }
        Ok(res)
    }
    #[cfg(windows)]
    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let mut dir = tokio::fs::read_dir(path).await?;
        let mut res = Vec::new();
        while let Some(entry) = dir.next_entry().await? {
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
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        Ok(buf)
    }

    async fn read_range(&self, path: &Path, start: u64, len: u64) -> Result<Vec<u8>, CoreError> {
        let mut f = File::open(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut buf = vec![0; len as usize];
        f.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let size = f
            .read(&mut buf)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        if size != (len as usize) {
            buf.resize(size, 0);
        }
        Ok(buf)
    }

    async fn exists(&self, path: &Path) -> Result<bool, CoreError> {
        tokio::fs::try_exists(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        FileNode::from_path(path.to_path_buf(), Some(self.reg.clone()))
    }

    /// Open a buffered, seekable reader over a local file.
    ///
    /// `BufReader<File>` satisfies `Read + BufRead + Seek` — the `Seek` impl
    /// on `BufReader` delegates to the inner `File` and clears the buffer.
    async fn open_reader(&self, path: &Path) -> Result<Box<dyn ReadSeek>, CoreError> {
        let file = std::fs::File::open(path)
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        Ok(Box::new(std::io::BufReader::new(file)))
    }

    /// Read the first `n_bytes` for MIME detection using a single `read_exact`.
    ///
    /// More efficient than the default `read_range` implementation because it
    /// opens the file once without seeking and reads exactly what is needed.
    ///
    /// # TODO
    /// - Open file with `tokio::fs::File::open(path)`
    /// - Allocate `buf: Vec<u8>` of capacity `n_bytes`
    /// - `file.read_exact(&mut buf)` — handle short reads (file smaller than n_bytes)
    ///   by resizing the buffer to the number of bytes actually read
    /// - Return `Ok(buf)`
    async fn read_header(&self, path: &Path, n_bytes: usize) -> Result<Vec<u8>, CoreError> {
        let mut f = File::open(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut buf = vec![0; n_bytes];
        let size = f
            .read_exact(&mut buf)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        if size != n_bytes {
            buf.resize(size, 0);
        }
        Ok(buf)
    }

    async fn write(&self, path: &Path, data: &[u8]) -> Result<(), CoreError> {
        tokio::fs::write(path, data)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn copy(&self, src: &Path, dst: &Path) -> Result<(), CoreError> {
        let meta = tokio::fs::metadata(src)
            .await
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))?;
        if meta.is_dir() {
            return Self::copy_dir(src, dst)
                .await
                .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()));
        }
        tokio::fs::copy(src, dst)
            .await
            .map(|_v| ())
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn rename(&self, src: &Path, dst: &Path) -> Result<(), CoreError> {
        tokio::fs::rename(src, dst)
            .await
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn delete(&self, path: &Path) -> Result<(), CoreError> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        if meta.is_dir() {
            tokio::fs::remove_dir_all(path).await
        } else {
            tokio::fs::remove_file(path).await
        }
        .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn mkdir(&self, path: &Path) -> Result<(), CoreError> {
        tokio::fs::create_dir_all(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }
}
impl LocalFs {
    async fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
        tokio::fs::create_dir_all(dst).await?;
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if entry.file_type().await?.is_dir() {
                Box::pin(Self::copy_dir(&src_path, &dst_path)).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
        Ok(())
    }
}
