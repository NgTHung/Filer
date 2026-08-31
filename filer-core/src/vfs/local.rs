use async_trait::async_trait;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryPageRequest, DirectoryPageResult, DirectoryPageState,
};
use crate::model::node::NodeEntry;
use crate::vfs::context::ProviderCx;
use crate::vfs::listing_stream::DirectoryStream;
use crate::vfs::local_listing::{LocalListingStream, read_entry};
use crate::vfs::provider::{
    Capabilities, FsProvider, ListingDetail, ListingOptions, ProviderPaging, ReadSeek,
    parse_offset_cursor, validate_page_limit,
};

/// Local filesystem provider
pub struct LocalFs {}

impl LocalFs {
    pub fn new() -> Self {
        Self {}
    }

    fn check_cancel(cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        if cx.cancel.is_some_and(crate::CancelSignal::is_cancelled) {
            return Err(CoreError::cancelled());
        }
        Ok(())
    }
}

impl Default for LocalFs {
    fn default() -> Self {
        Self::new()
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

    fn paging(&self) -> ProviderPaging {
        ProviderPaging::Native
    }

    async fn open_listing(
        &self,
        path: &Path,
        options: ListingOptions,
        cx: &ProviderCx<'_>,
    ) -> Result<Option<Box<dyn DirectoryStream>>, CoreError> {
        Self::check_cancel(cx)?;
        let stream = LocalListingStream::open(path, options).await?;
        Ok(Some(Box::new(stream)))
    }

    /// List directory contents using only `d_type` from the dirent — no stat per entry.
    ///
    /// Fields that require stat (`size`, timestamps, permissions) are
    /// left at zero/default. Use `list_with_meta` when those fields are needed.
    async fn list(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<NodeEntry>, CoreError> {
        Self::check_cancel(cx)?;
        let mut dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?
        {
            Self::check_cancel(cx)?;
            if let Some(entry) = read_entry(entry, ListingDetail::Fast).await {
                res.push(entry);
            }
        }
        Ok(res)
    }

    async fn list_with_options(
        &self,
        path: &Path,
        options: ListingOptions,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<NodeEntry>, CoreError> {
        match options.detail {
            ListingDetail::Fast => self.list(path, cx).await,
            ListingDetail::Metadata => self.list_with_meta(path, cx).await,
        }
    }

    async fn list_page(
        &self,
        path: &Path,
        request: DirectoryPageRequest,
        cx: &ProviderCx<'_>,
    ) -> Result<DirectoryPageResult, CoreError> {
        Self::check_cancel(cx)?;
        validate_page_limit(request.limit)?;
        let start = parse_offset_cursor(request.cursor.as_ref())?;
        let mut dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut seen = 0usize;
        let mut entries = Vec::new();
        let mut has_more = false;

        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?
        {
            Self::check_cancel(cx)?;
            if seen < start {
                seen += 1;
                continue;
            }

            if entries.len() >= request.limit {
                has_more = true;
                break;
            }

            if let Some(entry) = read_entry(entry, request.listing.detail).await {
                entries.push(entry);
            }
            seen += 1;
        }

        let state = if has_more {
            DirectoryPageState::partial(
                entries.len(),
                None,
                DirectoryCursor((start + entries.len()).to_string()),
            )
        } else {
            DirectoryPageState::complete(entries.len(), None)
        };
        Ok(DirectoryPageResult { entries, state })
    }

    async fn read(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Self::check_cancel(cx)?;
        let mut f = File::open(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        Self::check_cancel(cx)?;
        Ok(buf)
    }

    async fn read_range(
        &self,
        path: &Path,
        start: u64,
        len: u64,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Self::check_cancel(cx)?;
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
        Self::check_cancel(cx)?;
        Ok(buf)
    }

    async fn exists(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<bool, CoreError> {
        Self::check_cancel(cx)?;
        tokio::fs::try_exists(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn metadata(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<NodeEntry, CoreError> {
        Self::check_cancel(cx)?;
        NodeEntry::from_path(path.to_path_buf())
    }

    /// Open a buffered, seekable reader over a local file.
    ///
    /// `BufReader<File>` satisfies `Read + BufRead + Seek` — the `Seek` impl
    /// on `BufReader` delegates to the inner `File` and clears the buffer.
    async fn open_reader(
        &self,
        path: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<Box<dyn ReadSeek>, CoreError> {
        Self::check_cancel(cx)?;
        let file = std::fs::File::open(path)
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        Ok(Box::new(std::io::BufReader::new(file)))
    }

    /// Read up to `n_bytes` from the file head for MIME detection.
    ///
    /// More efficient than the default `read_range` because it opens the file
    /// once without seeking. The fill loop tolerates short reads and returns
    /// the bytes actually available, so files smaller than `n_bytes` still get
    /// magic-byte detection instead of an `UnexpectedEof` error.
    async fn read_header(
        &self,
        path: &Path,
        n_bytes: usize,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Self::check_cancel(cx)?;
        let mut f = File::open(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut buf = vec![0u8; n_bytes];
        let mut filled = 0;
        while filled < n_bytes {
            let n = f
                .read(&mut buf[filled..])
                .await
                .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
            if n == 0 {
                break;
            }
            filled += n;
            Self::check_cancel(cx)?;
        }
        buf.truncate(filled);
        Ok(buf)
    }

    async fn write(&self, path: &Path, data: &[u8], cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Self::check_cancel(cx)?;
        tokio::fs::write(path, data)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
    }

    async fn copy(&self, src: &Path, dst: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Self::check_cancel(cx)?;
        tokio::fs::copy(src, dst)
            .await
            .map(|_| ())
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn rename(&self, src: &Path, dst: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Self::check_cancel(cx)?;
        tokio::fs::rename(src, dst)
            .await
            .map_err(|e| CoreError::from_io_error(e, src.to_path_buf()))
    }

    async fn delete(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Self::check_cancel(cx)?;
        // Try file first (common case, no stat needed). Fall back to dir removal
        // if it fails — covers directories and edge cases like non-empty dirs.
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(_) => tokio::fs::remove_dir_all(path)
                .await
                .map_err(|e| CoreError::from_io_error(e, path.to_path_buf())),
        }
    }

    async fn mkdir(&self, path: &Path, cx: &ProviderCx<'_>) -> Result<(), CoreError> {
        Self::check_cancel(cx)?;
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
    pub async fn list_with_meta(
        &self,
        path: &Path,
        cx: &ProviderCx<'_>,
    ) -> Result<Vec<NodeEntry>, CoreError> {
        Self::check_cancel(cx)?;
        let mut dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        let mut res = Vec::new();
        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?
        {
            Self::check_cancel(cx)?;
            if let Some(entry) = read_entry(entry, ListingDetail::Metadata).await {
                res.push(entry);
            }
        }
        Ok(res)
    }
}
