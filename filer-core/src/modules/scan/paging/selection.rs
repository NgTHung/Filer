//! # Bounded Page Selection
//!
//! Selects one ordered page out of a full walk while retaining only the page
//! plus one lookahead row. Ordering and grouping need every row before the
//! first page is correct, so this path trades a full walk for bounded memory.
//! Streaming modes use [`super::stream`] instead.

use crate::model::directory::DEFAULT_DIRECTORY_PAGE_SIZE;
use crate::model::node::NodeEntry;
use crate::pipeline::{Pipeline, PipelineConfig, compare_nodes};
use crate::vfs::context::ProviderCx;

const CANCELLATION_CHECK_INTERVAL: usize = 256;

pub(crate) struct PageSelection<'a> {
    pub(crate) entries: Vec<NodeEntry>,
    pub(crate) total_matches: usize,
    /// Whether the window dropped a row, meaning `entries` is not the whole tail.
    pub(crate) overflowed: bool,
    /// Whether the window holds rows beyond the has-more probe, worth storing.
    pub(crate) retains: bool,
    window: usize,
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
            entries: Vec::with_capacity(window.min(DEFAULT_DIRECTORY_PAGE_SIZE)),
            total_matches: 0,
            overflowed: false,
            retains: lookahead > 0,
            window,
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
            let index = self
                .entries
                .binary_search_by(|existing| compare_nodes(self.pipeline_config, existing, &entry))
                .unwrap_or_else(|index| index);
            self.entries.insert(index, entry);
            if self.entries.len() > self.window {
                self.entries.pop();
                self.overflowed = true;
            }
        }
        !cx.is_cancelled()
    }
}
