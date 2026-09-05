//! # Streaming Page Assembly
//!
//! Builds a page by pulling only as many provider rows as the page needs, then
//! keeps the walk open for the next request. This is correct only when the
//! pipeline preserves provider order, so [`crate::pipeline::PipelinePagingMode`]
//! decides which chains come here.
//!
//! Rows pulled past the page boundary stay in `pending` rather than being
//! discarded. Re-reading them is impossible once the stream has advanced, and
//! dropping them would silently lose entries.

use std::collections::VecDeque;

use crate::errors::CoreError;
use crate::model::directory::DEFAULT_DIRECTORY_PAGE_SIZE;
use crate::model::node::NodeEntry;
use crate::pipeline::{Pipeline, PipelineConfig, PipelinePagingMode};
use crate::vfs::context::ProviderCx;
use crate::vfs::listing_stream::DirectoryStream;

/// Whether a configuration can be paged without materializing the directory.
pub(crate) fn streams_pages(config: &PipelineConfig) -> bool {
    matches!(
        config.paging_mode(),
        PipelinePagingMode::ProviderPage | PipelinePagingMode::FilteredPage
    )
}

/// A page taken from a live provider walk, plus the walk to store for the next.
pub(crate) struct StreamedPage {
    pub(crate) entries: Vec<NodeEntry>,
    pub(crate) stream: Box<dyn DirectoryStream>,
    pub(crate) pending: VecDeque<NodeEntry>,
    pub(crate) exhausted: bool,
}

impl StreamedPage {
    /// Whether the chain has more rows after this page.
    pub(crate) fn has_more(&self) -> bool {
        !self.pending.is_empty() || !self.exhausted
    }
}

/// How many rows to request per batch.
///
/// Without a filter every row reaches the page, so asking for exactly what is
/// missing keeps provider work equal to the page size. A filter can drop any
/// number of rows, so a chain that asked for the shortfall alone would issue a
/// batch per surviving row across a sparse directory.
fn batch_size(config: &PipelineConfig, shortfall: usize) -> usize {
    match config.paging_mode() {
        PipelinePagingMode::ProviderPage => shortfall.max(1),
        _ => shortfall.max(DEFAULT_DIRECTORY_PAGE_SIZE),
    }
}

/// Pull rows until the page is full, the directory ends, or the caller cancels.
///
/// Returns `Ok(None)` when cancellation interrupted the walk.
pub(crate) async fn take_page(
    mut stream: Box<dyn DirectoryStream>,
    mut pending: VecDeque<NodeEntry>,
    mut exhausted: bool,
    limit: usize,
    config: &PipelineConfig,
    cx: &ProviderCx<'_>,
) -> Result<Option<StreamedPage>, CoreError> {
    let pipeline = Pipeline::from_config(config);
    // One row past the page proves whether a continuation is warranted without
    // opening a second chain to find out.
    let target = limit.saturating_add(1);

    while pending.len() < target && !exhausted {
        if cx.is_cancelled() {
            return Ok(None);
        }
        let shortfall = target - pending.len();
        let batch = stream.next_batch(batch_size(config, shortfall), cx).await?;
        exhausted = batch.end_of_directory;
        for entry in batch.entries {
            if let Some(kept) = pipeline.filter_entry(entry) {
                pending.push_back(kept);
            }
        }
    }
    if cx.is_cancelled() {
        return Ok(None);
    }

    let taken = limit.min(pending.len());
    let entries: Vec<NodeEntry> = pending.drain(..taken).collect();
    Ok(Some(StreamedPage {
        entries,
        stream,
        pending,
        exhausted,
    }))
}
