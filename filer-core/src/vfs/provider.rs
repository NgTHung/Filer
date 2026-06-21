use async_trait::async_trait;
use std::path::Path;

use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::node::FileNode;
use crate::vfs::context::ProviderCx;
use serde::{Deserialize, Serialize};

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

/// Describes whether a provider can fetch pages without materializing a full listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPaging {
    Fallback,
    Native,
}

/// Controls how much metadata a directory listing should populate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingDetail {
    /// Fast listing using directory-entry type data only.
    #[default]
    Fast,
    /// Listing with per-entry metadata such as size, timestamps, and permissions.
    Metadata,
}

/// Options for provider directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ListingOptions {
    #[serde(default)]
    pub detail: ListingDetail,
}

impl ListingOptions {
    pub const fn fast() -> Self {
        Self {
            detail: ListingDetail::Fast,
        }
    }

    pub const fn metadata() -> Self {
        Self {
            detail: ListingDetail::Metadata,
        }
    }
}

/// Trait for filesystem backends
#[async_trait]
pub trait FsProvider: Send + Sync {
    /// Unique scheme for this provider (e.g., "file", "zip", "sftp")
    fn scheme(&self) -> &'static str;

    /// Provider capabilities
    fn capabilities(&self) -> Capabilities;

    fn paging(&self) -> ProviderPaging {
        ProviderPaging::Fallback
    }

    /// List contents of a directory
    async fn list(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<FileNode>, CoreError>;

    /// List contents of a directory with explicit detail options.
    ///
    /// Providers that do not distinguish listing detail can rely on this
    /// default implementation. LocalFs overrides it so callers can choose
    /// cheap rows or stat-backed metadata rows.
    async fn list_with_options(
        &self,
        path: &Path,
        _options: ListingOptions,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<FileNode>, CoreError> {
        self.list(path, cx).await
    }

    /// List a provider-owned page of directory entries.
    ///
    /// The default implementation falls back to a full listing and slices it.
    /// Providers with native cursors should override this to avoid materializing
    /// the whole directory.
    async fn list_page(
        &self,
        path: &Path,
        request: DirectoryPageRequest,
        cx: &ProviderCx<'_>,
    ) -> Result<DirectoryPageResult, CoreError> {
        validate_page_limit(request.limit)?;
        let entries = self.list_with_options(path, request.listing, cx).await?;
        let start = parse_offset_cursor(request.cursor.as_ref())?;
        let end = start.saturating_add(request.limit).min(entries.len());
        let page_entries = if start < entries.len() {
            entries[start..end].to_vec()
        } else {
            Vec::new()
        };
        let state = if end < entries.len() {
            DirectoryPageState::partial(
                page_entries.len(),
                Some(entries.len()),
                DirectoryCursor(end.to_string()),
            )
        } else {
            DirectoryPageState::complete(page_entries.len(), Some(entries.len()))
        };
        Ok(DirectoryPageResult {
            entries: page_entries,
            state,
        })
    }

    /// Read file contents
    async fn read(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError>;

    /// Read partial file contents
    async fn read_range(
        &self,
        path: &Path,
        start: u64,
        len: u64,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError>;

    /// Check if path exists
    async fn exists(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<bool, CoreError>;

    /// Get metadata for a path
    async fn metadata(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<FileNode, CoreError>;

    /// Read the first `n_bytes` of a file for MIME magic-byte detection.
    ///
    /// The default implementation delegates to `read_range(path, 0, n_bytes)`.
    /// Local providers should override this with a single `pread` / `read_exact`
    /// call to avoid the seek overhead of the generic implementation.
    ///
    /// Remote providers (S3, WebDAV, SFTP) should return `Err` to signal that
    /// magic-byte detection is unavailable — callers will then fall back to
    /// extension-only detection regardless of the requested `DetectionStrategy`.
    async fn read_header(
        &self,
        path: &Path,
        n_bytes: usize,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        self.read_range(path, 0, n_bytes as u64, cx).await
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
    async fn open_reader(
        &self,
        path: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<Box<dyn ReadSeek>, CoreError> {
        let bytes = self.read(path, cx).await?;
        Ok(Box::new(std::io::Cursor::new(bytes)))
    }

    async fn write(&self, path: &Path, _data: &[u8], _cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Err(CoreError::permission_denied(path.to_path_buf()))
    }
    async fn copy(&self, _src: &Path, dst: &Path, _cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Err(CoreError::permission_denied(dst.to_path_buf()))
    }
    async fn rename(&self, _src: &Path, dst: &Path, _cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Err(CoreError::permission_denied(dst.to_path_buf()))
    }
    async fn delete(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Err(CoreError::permission_denied(path.to_path_buf()))
    }
    async fn mkdir(&self, path: &Path, _cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Err(CoreError::permission_denied(path.to_path_buf()))
    }
}

pub(crate) fn parse_offset_cursor(cursor: Option<&DirectoryCursor>) -> Result<usize, CoreError> {
    match cursor {
        None => Ok(0),
        Some(cursor) => cursor.0.parse::<usize>().map_err(|_| {
            CoreError::invalid_input(format!("Invalid directory cursor: {}", cursor.0))
        }),
    }
}

pub(crate) fn validate_page_limit(limit: usize) -> Result<(), CoreError> {
    if limit == 0 {
        return Err(CoreError::invalid_input(
            "Directory page limit must be greater than zero",
        ));
    }
    Ok(())
}
