//! # Local Directory Streaming
//!
//! Backs [`crate::vfs::listing_stream::DirectoryStream`] with a retained
//! `tokio::fs::ReadDir`, so a paged local listing resumes where it stopped
//! instead of re-reading the prefix an offset cursor would skip.
//!
//! The entry conversion here is the single place that turns a `DirEntry` into a
//! [`NodeEntry`], so the cheap `d_type` path and the stat-backed path cannot
//! drift apart between the listing, paging, and streaming call sites.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs::{DirEntry, ReadDir};

use crate::errors::CoreError;
use crate::model::node::NodeEntry;
use crate::vfs::context::ProviderCx;
use crate::vfs::listing_stream::{DirectoryStream, ListingBatch};
use crate::vfs::provider::{ListingDetail, ListingOptions};

/// Convert one directory entry, or skip it when its metadata cannot be read.
///
/// A single unreadable entry must not fail the whole listing, because a
/// directory the user can browse often contains entries they cannot stat.
pub(crate) async fn read_entry(entry: DirEntry, detail: ListingDetail) -> Option<NodeEntry> {
    let path = entry.path();
    match detail {
        ListingDetail::Fast => match entry.file_type().await {
            Ok(file_type) => Some(NodeEntry::from_dir_entry(path, file_type)),
            Err(e) => {
                tracing::debug!(path = %path.display(), error = %e, "skipping entry in listing");
                None
            }
        },
        ListingDetail::Metadata => match entry.metadata().await {
            Ok(meta) => match NodeEntry::from_metadata(meta, path.clone()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    tracing::debug!(path = %path.display(), error = %e, "skipping entry in listing");
                    None
                }
            },
            Err(e) => {
                tracing::debug!(path = %path.display(), error = %e, "skipping entry metadata");
                None
            }
        },
    }
}

/// A local directory walk that holds its `ReadDir` between batches.
pub struct LocalListingStream {
    dir: ReadDir,
    path: PathBuf,
    detail: ListingDetail,
    exhausted: bool,
}

impl LocalListingStream {
    pub(crate) async fn open(path: &Path, options: ListingOptions) -> Result<Self, CoreError> {
        let dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| CoreError::from_io_error(e, path.to_path_buf()))?;
        Ok(Self {
            dir,
            path: path.to_path_buf(),
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
        if cx.is_cancelled() {
            return Err(CoreError::cancelled());
        }
        if self.exhausted {
            return Ok(ListingBatch::final_batch(Vec::new()));
        }

        let mut entries = Vec::with_capacity(max);
        while entries.len() < max {
            if cx.is_cancelled() {
                return Err(CoreError::cancelled());
            }
            let next = self
                .dir
                .next_entry()
                .await
                .map_err(|e| CoreError::from_io_error(e, self.path.clone()))?;
            let Some(entry) = next else {
                self.exhausted = true;
                return Ok(ListingBatch::final_batch(entries));
            };
            if let Some(entry) = read_entry(entry, self.detail).await {
                entries.push(entry);
            }
        }
        Ok(ListingBatch::partial(entries))
    }
}
