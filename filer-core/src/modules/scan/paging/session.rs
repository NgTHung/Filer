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
            Continuation::Stream { .. } => None,
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
}

impl PagingSessionStore {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            eviction_order: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    pub(crate) fn get(&self, cursor: &str) -> Option<&PagingSession> {
        self.sessions.get(cursor)
    }

    pub(crate) fn clear_owner(&mut self, owner: SessionId) {
        self.sessions.retain(|_, state| state.owner != owner);
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
            self.sessions.remove(&oldest);
        }
        self.eviction_order.push_back(cursor.clone());
        self.sessions.insert(cursor, state);
    }

    pub(crate) fn remove(&mut self, cursor: &str) -> Option<PagingSession> {
        let state = self.sessions.remove(cursor)?;
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
