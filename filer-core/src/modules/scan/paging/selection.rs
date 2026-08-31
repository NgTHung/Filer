//! # Bounded Page Selection
//!
//! Selects the ordered rows a walked chain needs while holding a bounded number
//! of them. Ordering and grouping need every row before the first page is
//! correct, so this path trades a full walk for bounded memory. Streaming modes
//! use [`super::stream`] instead.
//!
//! Rows accumulate into a buffer that is sorted and trimmed to the window
//! whenever it fills. Keeping the window sorted by inserting each row in place
//! instead costs a memmove per row, which is cheap for a page-sized window and
//! quadratic once the window is large enough to retain a whole directory.

use crate::model::directory::DEFAULT_DIRECTORY_PAGE_SIZE;
use crate::model::node::NodeEntry;
use crate::pipeline::{Pipeline, PipelineConfig, compare_nodes};
use crate::vfs::context::ProviderCx;

const CANCELLATION_CHECK_INTERVAL: usize = 256;

/// The ordered rows one walk selected.
pub(crate) struct SelectedRows {
    pub(crate) entries: Vec<NodeEntry>,
    pub(crate) total_matches: usize,
    /// Whether the window dropped a row, so `entries` is not the whole tail.
    pub(crate) overflowed: bool,
    /// Whether the window holds rows beyond the has-more probe, worth storing.
    pub(crate) retains: bool,
}

pub(crate) struct PageSelection<'a> {
    buffer: Vec<NodeEntry>,
    pub(crate) total_matches: usize,
    overflowed: bool,
    retains: bool,
    window: usize,
    flush_at: usize,
    after: Option<NodeEntry>,
    pipeline_config: &'a PipelineConfig,
    pipeline: Pipeline,
}

impl<'a> PageSelection<'a> {
    /// Keep `lookahead` ordered rows past the page so continuations can be
    /// served without walking the directory again.
    pub(crate) fn with_lookahead(
        limit: usize,
        lookahead: usize,
        after: Option<NodeEntry>,
        pipeline_config: &'a PipelineConfig,
    ) -> Self {
        // At least one row past the page decides whether a continuation is
        // warranted; anything beyond that is retention for the next page.
        let window = limit.saturating_add(lookahead.max(1));
        Self {
            buffer: Vec::with_capacity(window.min(DEFAULT_DIRECTORY_PAGE_SIZE)),
            total_matches: 0,
            overflowed: false,
            retains: lookahead > 0,
            window,
            // Trimming at twice the window amortizes each sort over at least a
            // window's worth of new rows.
            flush_at: window.saturating_mul(2),
            after,
            pipeline_config,
            pipeline: Pipeline::from_config(pipeline_config),
        }
    }

    pub(crate) fn extend<I>(&mut self, entries: I, cx: &ProviderCx<'_>) -> bool
    where
        I: IntoIterator<Item = NodeEntry>,
    {
        if cx.is_cancelled() {
            return false;
        }
        for (index, entry) in entries.into_iter().enumerate() {
            if index % CANCELLATION_CHECK_INTERVAL == 0 && cx.is_cancelled() {
                return false;
            }
            let mut filtered = self.pipeline.execute_flat(vec![entry]);
            let Some(entry) = filtered.pop() else {
                continue;
            };
            self.total_matches += 1;
            if self
                .after
                .as_ref()
                .is_some_and(|after| compare_nodes(self.pipeline_config, &entry, after).is_le())
            {
                continue;
            }
            self.buffer.push(entry);
            if self.buffer.len() >= self.flush_at {
                self.trim();
            }
        }
        !cx.is_cancelled()
    }

    /// Order the buffer and drop everything past the window.
    fn trim(&mut self) {
        self.buffer
            .sort_unstable_by(|left, right| compare_nodes(self.pipeline_config, left, right));
        if self.buffer.len() > self.window {
            self.buffer.truncate(self.window);
            self.overflowed = true;
        }
    }

    pub(crate) fn finish(mut self) -> SelectedRows {
        self.trim();
        SelectedRows {
            entries: self.buffer,
            total_matches: self.total_matches,
            overflowed: self.overflowed,
            retains: self.retains,
        }
    }
}
