//! # Local Directory Listing
//!
//! Reads local directories for [`crate::LocalFs`]. Every full listing, offset
//! page, and stream batch runs in one blocking task that pulls entries from a
//! `std::fs::ReadDir` and reads each entry's type or metadata in the same task.
//! `tokio::fs` sends every `DirEntry::metadata` call to the blocking pool on its
//! own, so a metadata page used to pay one pool hop per row for the same
//! platform calls.
//!
//! [`LocalListingStream`] keeps its `ReadDir` between batches, so a paged
//! listing resumes where it stopped instead of re-reading the prefix an offset
//! cursor would skip. The entry conversion here is the single place that turns
//! a `DirEntry` into a [`NodeEntry`], so the cheap file-type path and the
//! stat-backed path cannot drift apart between listing, paging, and streaming.
//!
//! ```
//! use filer_core::{FsProvider, ListingOptions, LocalFs, ProviderCx};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tokio::runtime::Runtime::new()?.block_on(async {
//! # let directory = tempfile::tempdir()?;
//! # std::fs::write(directory.path().join("report.txt"), "0123")?;
//! let fs = LocalFs::new();
//! let cx = ProviderCx::none();
//! let mut stream = fs
//!     .open_listing(directory.path(), ListingOptions::metadata(), &cx)
//!     .await?
//!     .ok_or("the local provider streams listings")?;
//! let batch = stream.next_batch(64, &cx).await?;
//! assert_eq!(batch.entries[0].size, 4);
//! assert!(batch.end_of_directory);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # })
//! # }
//! ```

use async_trait::async_trait;
use std::fs::{DirEntry, ReadDir};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::errors::CoreError;
use crate::model::cancel::CancelSignal;
use crate::model::directory::{
    DEFAULT_DIRECTORY_PAGE_SIZE, DirectoryCursor, DirectoryPageResult, DirectoryPageState,
};
use crate::model::node::NodeEntry;
use crate::vfs::context::ProviderCx;
use crate::vfs::listing_stream::{DirectoryStream, ListingBatch};
use crate::vfs::local::LOCAL_SCHEME;
use crate::vfs::provider::{ListingDetail, ListingOptions};

/// The caller's cancellation and deadline, owned so a blocking read can poll them.
///
/// A blocking task keeps running after the future awaiting it is dropped, so it
/// has to stop itself instead of finishing a page nobody will receive.
pub(crate) struct ListingStop {
    cancel: Option<CancelSignal>,
    deadline: Option<Instant>,
}

impl ListingStop {
    pub(crate) fn new(cx: &ProviderCx<'_>) -> Self {
        Self {
            cancel: cx.cancel.cloned(),
            deadline: cx.deadline,
        }
    }

    fn check(&self) -> Result<(), CoreError> {
        if self.cancel.as_ref().is_some_and(CancelSignal::is_cancelled) {
            return Err(CoreError::cancelled());
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(CoreError::provider_timed_out(
                LOCAL_SCHEME,
                format!("Provider '{LOCAL_SCHEME}' timed out"),
            ));
        }
        Ok(())
    }
}

/// Rows produced by one blocking read.
pub(crate) struct BatchRead {
    pub(crate) entries: Vec<NodeEntry>,
    /// Directory entries consumed, including ones skipped as unreadable.
    pub(crate) consumed: usize,
    pub(crate) end_of_directory: bool,
}

/// Read up to `max` rows from `dir` on the calling thread.
///
/// Callers run this inside a blocking task, which is why it polls `stop`
/// before every entry.
pub(crate) fn read_batch(
    dir: &mut ReadDir,
    path: &Path,
    detail: ListingDetail,
    max: usize,
    stop: &ListingStop,
) -> Result<BatchRead, CoreError> {
    let mut entries = Vec::with_capacity(max.min(DEFAULT_DIRECTORY_PAGE_SIZE));
    let mut consumed = 0;
    while entries.len() < max {
        stop.check()?;
        let Some(next) = dir.next() else {
            return Ok(BatchRead {
                entries,
                consumed,
                end_of_directory: true,
            });
        };
        let entry = next.map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        consumed += 1;
        if let Some(entry) = read_entry(&entry, detail) {
            entries.push(entry);
        }
    }
    Ok(BatchRead {
        entries,
        consumed,
        end_of_directory: false,
    })
}

/// Convert one directory entry, or skip it when its metadata cannot be read.
///
/// A single unreadable entry must not fail the whole listing, because a
/// directory the user can browse often contains entries they cannot stat.
fn read_entry(entry: &DirEntry, detail: ListingDetail) -> Option<NodeEntry> {
    let row = match detail {
        ListingDetail::Fast => entry
            .file_type()
            .map(|file_type| NodeEntry::from_dir_entry(entry.path(), file_type))
            .map_err(CoreError::other),
        ListingDetail::Metadata => entry
            .metadata()
            .map_err(CoreError::other)
            .and_then(|meta| NodeEntry::from_metadata(meta, entry.path())),
    };
    match row {
        Ok(row) => Some(row),
        Err(e) => {
            tracing::debug!(path = %entry.path().display(), error = %e, "skipping entry in listing");
            None
        }
    }
}

fn open_dir(path: &Path) -> Result<ReadDir, CoreError> {
    std::fs::read_dir(path).map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))
}

async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CoreError> + Send + 'static,
) -> Result<T, CoreError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| CoreError::actor("local_fs", e.to_string()))?
}

/// List every row of `path` in one blocking task.
pub(crate) async fn list_all(
    path: &Path,
    detail: ListingDetail,
    cx: &ProviderCx<'_>,
) -> Result<Vec<NodeEntry>, CoreError> {
    let stop = ListingStop::new(cx);
    stop.check()?;
    let path = path.to_path_buf();
    run_blocking(move || {
        let mut dir = open_dir(&path)?;
        Ok(read_batch(&mut dir, &path, detail, usize::MAX, &stop)?.entries)
    })
    .await
}

/// Read the page that starts `start` entries into `path`, in one blocking task.
///
/// The cursor counts directory entries rather than rows, so an entry skipped as
/// unreadable does not shift the next page back onto rows already returned.
pub(crate) async fn list_offset_page(
    path: &Path,
    detail: ListingDetail,
    start: usize,
    limit: usize,
    cx: &ProviderCx<'_>,
) -> Result<DirectoryPageResult, CoreError> {
    let stop = ListingStop::new(cx);
    stop.check()?;
    let path = path.to_path_buf();
    run_blocking(move || {
        let io_error = |e| CoreError::from_io_error(e, path.clone());
        let mut dir = open_dir(&path)?;
        for skipped in dir.by_ref().take(start) {
            stop.check()?;
            skipped.map_err(io_error)?;
        }
        let batch = read_batch(&mut dir, &path, detail, limit, &stop)?;
        // One raw entry past the page proves a continuation exists.
        let has_more =
            !batch.end_of_directory && dir.next().transpose().map_err(io_error)?.is_some();
        let page_count = batch.entries.len();
        let state = if has_more {
            DirectoryPageState::partial(
                page_count,
                None,
                DirectoryCursor((start + batch.consumed).to_string()),
            )
        } else {
            DirectoryPageState::complete(page_count, None)
        };
        Ok(DirectoryPageResult {
            entries: batch.entries,
            state,
        })
    })
    .await
}

/// A local directory walk that holds its `ReadDir` between batches.
pub struct LocalListingStream {
    /// Empty once the walk ends, or once a batch was abandoned mid-read and took
    /// its rows with it.
    dir: Option<ReadDir>,
    path: Arc<Path>,
    detail: ListingDetail,
    exhausted: bool,
}

impl LocalListingStream {
    pub(crate) async fn open(path: &Path, options: ListingOptions) -> Result<Self, CoreError> {
        let path: Arc<Path> = Arc::from(path);
        let dir = {
            let path = Arc::clone(&path);
            run_blocking(move || open_dir(&path)).await?
        };
        Ok(Self {
            dir: Some(dir),
            path,
            detail: options.detail,
            exhausted: false,
        })
    }
}

#[async_trait]
impl DirectoryStream for LocalListingStream {
    async fn next_batch(
        &mut self,
        max: usize,
        cx: &ProviderCx<'_>,
    ) -> Result<ListingBatch, CoreError> {
        let stop = ListingStop::new(cx);
        stop.check()?;
        if self.exhausted {
            return Ok(ListingBatch::final_batch(Vec::new()));
        }
        // Resuming after a lost batch would silently skip its rows, so the
        // caller has to reopen the listing instead.
        let Some(mut dir) = self.dir.take() else {
            return Err(CoreError::io(
                self.path.to_path_buf(),
                "Directory listing was interrupted mid-batch",
            ));
        };

        let path = Arc::clone(&self.path);
        let detail = self.detail;
        let (dir, batch) = run_blocking(move || {
            let batch = read_batch(&mut dir, &path, detail, max, &stop)?;
            Ok((dir, batch))
        })
        .await?;

        if batch.end_of_directory {
            self.exhausted = true;
            return Ok(ListingBatch::final_batch(batch.entries));
        }
        self.dir = Some(dir);
        Ok(ListingBatch::partial(batch.entries))
    }
}
