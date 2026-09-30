//! # Retained Ordered Pages
//!
//! Sorting and grouping need every row before the first page is correct, so an
//! ordered chain must walk the directory once. It should not walk it again per
//! page. The first walk keeps the rows past the page it returned, and later
//! pages come straight out of that tail.
//!
//! Retention is bounded twice: one chain keeps at most
//! [`super::session::MAX_RETAINED_ROWS_PER_CHAIN`] rows, and all chains share a
//! budget. A chain that cannot retain falls back to the keyset rewalk, which is
//! slower but correct.

use std::collections::VecDeque;

use crate::model::node::NodeEntry;

/// A page taken from rows a previous walk already ordered.
pub(crate) struct RetainedPage {
    pub(crate) entries: Vec<NodeEntry>,
    pub(crate) rows: VecDeque<NodeEntry>,
    pub(crate) tail_complete: bool,
    /// Whether `rows` may be stored, or exists only to prove more rows remain.
    pub(crate) retain: bool,
}

impl RetainedPage {
    pub(crate) fn take(mut rows: VecDeque<NodeEntry>, tail_complete: bool, limit: usize) -> Self {
        let taken = limit.min(rows.len());
        let entries = rows.drain(..taken).collect();
        Self {
            entries,
            rows,
            tail_complete,
            retain: true,
        }
    }

    /// A tail known to be partial always has more rows behind it.
    pub(crate) fn has_more(&self) -> bool {
        !self.rows.is_empty() || !self.tail_complete
    }
}
