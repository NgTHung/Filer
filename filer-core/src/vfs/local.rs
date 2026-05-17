use async_trait::async_trait;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::errors::CoreError;
use crate::model::node::FileNode;
use crate::model::registry::NodeRegistry;
use crate::vfs::provider::{Capabilities, FsProvider, ListingDetail, ListingOptions, ReadSeek};

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

    /// List directory contents using only `d_type` from the dirent — no stat per entry.
    ///
    /// `FileNode` fields that require stat (`size`, timestamps, permissions) are
    /// left at zero/default. Use `list_with_meta` when those fields are needed.
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
            match entry.file_type().await {
                Ok(ft) => res.push(FileNode::from_dir_entry(
                    entry.path(),
                    ft,
                    Some(self.reg.clone()),
                )),
                Err(e) => {
                    tracing::debug!(path = %entry.path().display(), error = %e, "skipping entry in listing");
                }
            }
        }
        Ok(res)
    }

    async fn list_with_options(
        &self,
        path: &Path,
        options: ListingOptions,
    ) -> Result<Vec<FileNode>, CoreError> {
        match options.detail {
            ListingDetail::Fast => self.list(path).await,
            ListingDetail::Metadata => self.list_with_meta(path).await,
        }
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
        tokio::fs::copy(src, dst)
            .await
            .map(|_| ())
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn rename(&self, src: &Path, dst: &Path) -> Result<(), CoreError> {
        tokio::fs::rename(src, dst)
            .await
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn delete(&self, path: &Path) -> Result<(), CoreError> {
        // Try file first (common case, no stat needed). Fall back to dir removal
        // if it fails — covers directories and edge cases like non-empty dirs.
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(_) => tokio::fs::remove_dir_all(path)
                .await
                .map_err(|e| CoreError::from_io_error(e, path.to_path_buf())),
        }
    }

    async fn mkdir(&self, path: &Path) -> Result<(), CoreError> {
        tokio::fs::create_dir_all(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }
}
impl LocalFs {
    /// List directory with full stat metadata per entry (size, timestamps, permissions).
    ///
    /// More expensive than `list()` — uses `entry.metadata()` which issues a
    /// stat syscall per entry. Use this when the UI needs to display file sizes
    /// or timestamps, not for internal walks (copy, delete, etc.).
    pub async fn list_with_meta(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        let mut dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?
        {
            let entry_path = entry.path();
            match entry.metadata().await {
                Ok(meta) => {
                    match FileNode::from_metadata(meta, entry_path.clone(), Some(self.reg.clone()))
                    {
                        Ok(node) => res.push(node),
                        Err(e) => {
                            tracing::debug!(path = %entry_path.display(), error = %e, "skipping entry in listing");
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(path = %entry_path.display(), error = %e, "skipping entry metadata");
                }
            }
        }
        Ok(res)
    }
}
