//! # Terminal browsing model
//!
//! Directory results stay usable while replacement requests run. The model
//! correlates core results before changing the view, so terminal rendering does
//! not need to reason about late events or single-use continuations.
//!
//! ```
//! use filer_core::{Location, LocationRef, PipelineConfig};
//! use filer_core::model::session::SessionId;
//! use filer_tui::BrowsingModel;
//!
//! let mut model = BrowsingModel::new(SessionId::new());
//! let request = model.load(
//!     LocationRef::from_location(&Location::local("/tmp")),
//!     PipelineConfig::with_default_sort(),
//! );
//! assert_eq!(model.pending().map(|pending| pending.request), Some(request.request));
//! ```

use filer_core::model::session::SessionId;
use filer_core::{DirectoryPageState, Event, LocationRef, NodeEntry, PipelineConfig, RequestId};

/// Intent for a runner to send to core, without performing I/O during updates.
#[derive(Debug, Clone)]
pub struct BrowseRequest {
    pub request: RequestId,
    pub location: LocationRef,
    pub pipeline: PipelineConfig,
    /// Disposable read work the runner can cancel after replacement.
    pub replaces: Option<RequestId>,
}

#[derive(Debug)]
pub struct DirectoryView {
    location: LocationRef,
    entries: Vec<NodeEntry>,
    pages: Vec<DirectoryPageState>,
}

impl DirectoryView {
    pub fn location(&self) -> &LocationRef {
        &self.location
    }

    pub fn entries(&self) -> &[NodeEntry] {
        &self.entries
    }

    pub fn pages(&self) -> &[DirectoryPageState] {
        &self.pages
    }
}

#[derive(Debug)]
pub struct BrowsingModel {
    session: SessionId,
    pending: Option<BrowseRequest>,
    view: Option<DirectoryView>,
}

impl BrowsingModel {
    pub fn new(session: SessionId) -> Self {
        Self {
            session,
            pending: None,
            view: None,
        }
    }

    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn pending(&self) -> Option<&BrowseRequest> {
        self.pending.as_ref()
    }

    pub fn view(&self) -> Option<&DirectoryView> {
        self.view.as_ref()
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    /// Replace read intent while keeping the last successfully loaded view.
    pub fn load(&mut self, location: LocationRef, pipeline: PipelineConfig) -> BrowseRequest {
        let request = BrowseRequest {
            request: RequestId::new(),
            location,
            pipeline,
            replaces: self.pending.as_ref().map(|pending| pending.request),
        };
        self.pending = Some(request.clone());
        request
    }

    /// Return whether an event changed browsing state.
    pub fn apply(&mut self, event: Event) -> bool {
        let Event::DirectoryPageLoaded {
            parent,
            groups,
            page,
            session,
            request,
        } = event
        else {
            return false;
        };
        let Some(pending) = &self.pending else {
            return false;
        };
        if session != self.session
            || request != pending.request
            || parent.identity() != pending.location.identity()
        {
            return false;
        }
        self.view = Some(DirectoryView {
            location: parent,
            entries: groups
                .groups
                .into_iter()
                .flat_map(|group| group.nodes)
                .collect(),
            pages: vec![page],
        });
        self.pending = None;
        true
    }
}
