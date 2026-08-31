//! # Paging Session Store
//!
//! Holds the continuation state behind a directory cursor. The store is bounded
//! so abandoned pagination cannot grow without limit, and eviction drops a
//! session's continuation state, which releases any provider handle it holds.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::model::directory::{DirectoryCursor, DirectoryPageRequest};
use crate::model::node::NodeEntry;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;
use crate::vfs::listing_stream::DirectoryStream;

pub(crate) const CURSOR_PREFIX: &str = "paging:v1:";
pub(crate) const DEFAULT_PAGING_SESSION_CAPACITY: usize = 256;

/// Rows one ordered chain may keep so its continuations skip the provider walk.
///
/// Sized to hold a large directory whole, which is the case a continuation
/// should never pay a second walk for. A chain past this keeps a bounded
/// prefix and rewalks from its keyset boundary for the rest.
pub(crate) const MAX_RETAINED_ROWS_PER_CHAIN: usize = 16_384;

/// Rows all chains together may keep, so session capacity bounds memory rather
/// than only session count.
pub(crate) const DEFAULT_RETAINED_ROW_BUDGET: usize = 32_768;

/// Where the next page of a chain resumes from.
pub(crate) enum Continuation {
    /// A comparator boundary over a full walk, used when ordering or grouping
    /// needs every row before the first page is correct. The row is boxed so a
    /// keyset chain does not make every stored session pay its size.
    Keyset { last: Box<NodeEntry> },
    /// A live provider walk plus the rows already pulled past the last page.
    Stream {
        stream: Box<dyn DirectoryStream>,
        pending: VecDeque<NodeEntry>,
        exhausted: bool,
    },
    /// Ordered rows already materialized past the last page, plus the keyset
    /// boundary to rewalk from once they run out.
    Ordered {
        rows: VecDeque<NodeEntry>,
        last: Box<NodeEntry>,
        /// Whether `rows` holds every remaining row of the chain.
        tail_complete: bool,
    },
}

impl Continuation {
    pub(crate) fn retained_rows(&self) -> usize {
        match self {
            Continuation::Ordered { rows, .. } => rows.len(),
            Continuation::Keyset { .. } | Continuation::Stream { .. } => 0,
        }
    }
}

/// Rows a previous walk ordered, ready to answer the next page.
pub(crate) struct RetainedChain {
    pub(crate) rows: VecDeque<NodeEntry>,
    pub(crate) tail_complete: bool,
    pub(crate) start_index: usize,
    pub(crate) total_count: Option<usize>,
}

pub(crate) struct PagingSession {
    pub(crate) owner: SessionId,
    pub(crate) path: PathBuf,
    pub(crate) request: DirectoryPageRequest,
    pub(crate) pipeline: PipelineConfig,
    pub(crate) continuation: Continuation,
    pub(crate) start_index: usize,
    /// Known only once a chain has observed the end of the directory.
    pub(crate) total_count: Option<usize>,
}

impl PagingSession {
    pub(crate) fn keyset_boundary(&self) -> Option<&NodeEntry> {
        match &self.continuation {
            Continuation::Keyset { last } => Some(last),
            Continuation::Ordered { last, .. } => Some(last),
            Continuation::Stream { .. } => None,
        }
    }

    /// Take the retained rows when they can answer a page of `limit` without
    /// walking, otherwise hand the session back so the caller can walk.
    ///
    /// A partial tail shorter than the page cannot be served alone, because
    /// rows exist past it that would be skipped. The session is boxed on the
    /// way back so the common success path stays cheap.
    pub(crate) fn into_retained(self, limit: usize) -> Result<RetainedChain, Box<Self>> {
        let servable = match &self.continuation {
            Continuation::Ordered {
                rows,
                tail_complete,
                ..
            } => rows.len() > limit || *tail_complete,
            _ => false,
        };
        if !servable {
            return Err(Box::new(self));
        }
        let start_index = self.start_index;
        let total_count = self.total_count;
        match self.continuation {
            Continuation::Ordered {
                rows,
                tail_complete,
                ..
            } => Ok(RetainedChain {
                rows,
                tail_complete,
                start_index,
                total_count,
            }),
            continuation => Err(Box::new(Self {
                continuation,
                start_index,
                total_count,
                ..self
            })),
        }
    }

    pub(crate) fn is_streaming(&self) -> bool {
        matches!(self.continuation, Continuation::Stream { .. })
    }

    pub(crate) fn into_continuation(self) -> Continuation {
        self.continuation
    }
}

pub(crate) struct PagingSessionStore {
    sessions: HashMap<String, PagingSession>,
    eviction_order: VecDeque<String>,
    capacity: usize,
    retained_rows: usize,
    retained_budget: usize,
}

impl PagingSessionStore {
    pub(crate) fn new(capacity: usize, retained_budget: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            eviction_order: VecDeque::new(),
            capacity: capacity.max(1),
            retained_rows: 0,
            retained_budget,
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_rows(&self) -> usize {
        self.retained_rows
    }

    /// How many rows a new chain may retain right now.
    pub(crate) fn retention_allowance(&self) -> usize {
        self.retained_budget
            .saturating_sub(self.retained_rows)
            .min(MAX_RETAINED_ROWS_PER_CHAIN)
    }

    pub(crate) fn get(&self, cursor: &str) -> Option<&PagingSession> {
        self.sessions.get(cursor)
    }

    pub(crate) fn clear_owner(&mut self, owner: SessionId) {
        let mut released = 0;
        self.sessions.retain(|_, state| {
            if state.owner == owner {
                released += state.continuation.retained_rows();
                return false;
            }
            true
        });
        self.retained_rows = self.retained_rows.saturating_sub(released);
        self.eviction_order
            .retain(|cursor| self.sessions.contains_key(cursor));
    }

    pub(crate) fn insert(&mut self, cursor: String, state: PagingSession) {
        if self.sessions.contains_key(&cursor) {
            self.remove(&cursor);
        }
        while self.sessions.len() >= self.capacity {
            let Some(oldest) = self.eviction_order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.sessions.remove(&oldest) {
                self.retained_rows = self
                    .retained_rows
                    .saturating_sub(evicted.continuation.retained_rows());
            }
        }
        self.retained_rows += state.continuation.retained_rows();
        self.eviction_order.push_back(cursor.clone());
        self.sessions.insert(cursor, state);
    }

    pub(crate) fn remove(&mut self, cursor: &str) -> Option<PagingSession> {
        let state = self.sessions.remove(cursor)?;
        self.retained_rows = self
            .retained_rows
            .saturating_sub(state.continuation.retained_rows());
        if let Some(index) = self
            .eviction_order
            .iter()
            .position(|candidate| candidate == cursor)
        {
            self.eviction_order.remove(index);
        }
        Some(state)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }
}

pub(crate) fn next_cursor() -> DirectoryCursor {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    DirectoryCursor(format!(
        "{CURSOR_PREFIX}{}",
        COUNTER.fetch_add(1, AtomicOrdering::Relaxed)
    ))
}
