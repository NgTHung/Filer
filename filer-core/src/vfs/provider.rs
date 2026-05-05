use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::model::node::FileNode;

/// A combined `Read + BufRead + Seek` object trait used by extraction crates.
///
/// `BufRead` is required by crates such as `kamadak-exif` (`read_from_container`).
///
/// - **Local provider**: backed by `BufReader<std::fs::File>` — OS-buffered, seekable.
/// - **Remote provider**: backed by `Cursor<Vec<u8>>` after a full `read()` fetch;
///   `Cursor` satisfies all three traits without an extra wrapper.
pub trait ReadSeek: std::io::Read + std::io::BufRead + std::io::Seek + Send {}
impl<T: std::io::Read + std::io::BufRead + std::io::Seek + Send> ReadSeek for T {}

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
    async fn exists(&self, path: &Path) -> Result<bool, CoreError>;

    /// Get metadata for a path
    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError>;

    /// Read the first `n_bytes` of a file for MIME magic-byte detection.
    ///
    /// The default implementation delegates to `read_range(path, 0, n_bytes)`.
    /// Local providers should override this with a single `pread` / `read_exact`
    /// call to avoid the seek overhead of the generic implementation.
    ///
    /// Remote providers (S3, WebDAV, SFTP) should return `Err` to signal that
    /// magic-byte detection is unavailable — callers will then fall back to
    /// extension-only detection regardless of the requested `DetectionStrategy`.
    async fn read_header(&self, path: &Path, n_bytes: usize) -> Result<Vec<u8>, CoreError> {
        self.read_range(path, 0, n_bytes as u64).await
    }

    /// Open a file as a synchronous `Read + Seek` reader.
    ///
    /// Extraction crates (`zip`, `kamadak-exif`, `id3`, `mp4parse`, `lopdf`)
    /// accept `impl Read` or `impl Read + Seek`. Using a real reader avoids
    /// buffering the entire file into a `Vec<u8>` when the provider can serve
    /// a seekable handle directly.
    ///
    /// **Local provider**: override to return `std::fs::File` — zero extra allocation.
    /// **Remote providers**: the default implementation fetches all bytes via
    /// `read()` and wraps them in `Cursor<Vec<u8>>`, which is seekable.
    async fn open_reader(&self, path: &Path) -> Result<Box<dyn ReadSeek>, CoreError> {
        let bytes = self.read(path).await?;
        Ok(Box::new(std::io::Cursor::new(bytes)))
    }

    async fn write(&self, path: &Path, _data: &[u8]) -> Result<(), CoreError> {
        Err(CoreError::PermissionDenied(path.to_path_buf()))
    }
    async fn copy(&self, _src: &Path, dst: &Path) -> Result<(), CoreError> {
        Err(CoreError::PermissionDenied(dst.to_path_buf()))
    }
    async fn rename(&self, _src: &Path, dst: &Path) -> Result<(), CoreError> {
        Err(CoreError::PermissionDenied(dst.to_path_buf()))
    }
    async fn delete(&self, path: &Path) -> Result<(), CoreError> {
        Err(CoreError::PermissionDenied(path.to_path_buf()))
    }
    async fn mkdir(&self, path: &Path) -> Result<(), CoreError> {
        Err(CoreError::PermissionDenied(path.to_path_buf()))
    }
}
