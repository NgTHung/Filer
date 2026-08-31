//! # Bounded Page Selection
//!
//! Selects one ordered page out of a full walk while retaining only the page
//! plus one lookahead row. Ordering and grouping need every row before the
//! first page is correct, so this path trades a full walk for bounded memory.
//! Streaming modes use [`super::stream`] instead.

use crate::model::node::NodeEntry;
use crate::pipeline::{Pipeline, PipelineConfig, compare_nodes};
use crate::vfs::context::ProviderCx;

const CANCELLATION_CHECK_INTERVAL: usize = 256;

pub(crate) struct PageSelection<'a> {
    pub(crate) entries: Vec<NodeEntry>,
    pub(crate) total_matches: usize,
    limit: usize,
    after: Option<NodeEntry>,
    pipeline_config: &'a PipelineConfig,
    pipeline: Pipeline,
}

impl<'a> PageSelection<'a> {
    pub(crate) fn new(
        limit: usize,
        after: Option<NodeEntry>,
        pipeline_config: &'a PipelineConfig,
    ) -> Self {
        Self {
            entries: Vec::with_capacity(limit.saturating_add(1)),
            total_matches: 0,
            limit,
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
            if self.entries.len() > self.limit.saturating_add(1) {
                self.entries.pop();
            }
        }
        !cx.is_cancelled()
    }
}
