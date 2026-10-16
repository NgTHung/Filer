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

use std::collections::HashSet;
use std::ops::Range;

use filer_core::model::session::SessionId;
use filer_core::{
    DirectoryCursor, DirectoryPageState, Event, LocationId, LocationRef, NodeEntry, PipelineConfig,
    RequestId,
};

/// Intent for a runner to send to core, without performing I/O during updates.
#[derive(Debug, Clone)]
pub struct BrowseRequest {
    pub request: RequestId,
    pub location: LocationRef,
    pub pipeline: PipelineConfig,
    pub cursor: Option<DirectoryCursor>,
    /// Disposable read work the runner can cancel after replacement.
    pub replaces: Option<RequestId>,
}

#[derive(Debug)]
pub struct DirectoryView {
    location: LocationRef,
    pipeline: PipelineConfig,
    entries: Vec<NodeEntry>,
    pages: Vec<DirectoryPageState>,
    visible_range: Range<usize>,
    continuation: Option<DirectoryCursor>,
    selected: HashSet<LocationId>,
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

    pub fn pipeline(&self) -> &PipelineConfig {
        &self.pipeline
    }

    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }

    pub fn visible_entries(&self) -> &[NodeEntry] {
        &self.entries[self.visible_range.clone()]
    }

    pub fn selected(&self) -> &HashSet<LocationId> {
        &self.selected
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
            cursor: None,
            replaces: self.pending.as_ref().map(|pending| pending.request),
        };
        if let Some(view) = &mut self.view {
            view.continuation = None;
        }
        self.pending = Some(request.clone());
        request
    }

    /// Consume a continuation before returning it, preventing repeated sends.
    pub fn continue_loading(&mut self) -> Option<BrowseRequest> {
        if self.is_loading() {
            return None;
        }
        let view = self.view.as_mut()?;
        let request = BrowseRequest {
            request: RequestId::new(),
            location: view.location.clone(),
            pipeline: view.pipeline.clone(),
            cursor: Some(view.continuation.take()?),
            replaces: None,
        };
        self.pending = Some(request.clone());
        Some(request)
    }

    pub fn set_visible_range(&mut self, range: Range<usize>) {
        if let Some(view) = &mut self.view {
            let start = range.start.min(view.entries.len());
            view.visible_range = start..range.end.max(start).min(view.entries.len());
        }
    }

    /// Toggle a loaded row's Location, independently of the visible range.
    pub fn toggle_selection(&mut self, index: usize) -> bool {
        let Some(view) = &mut self.view else {
            return false;
        };
        let Some(entry) = view.entries.get(index) else {
            return false;
        };
        let identity = entry.location.identity();
        if !view.selected.insert(identity) {
            view.selected.remove(&identity);
        }
        true
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
        let entries = groups.groups.into_iter().flat_map(|group| group.nodes);
        let continuation = page.next_cursor.clone();
        if pending.cursor.is_some() {
            if let Some(view) = &mut self.view {
                view.entries.extend(entries);
                view.pages.push(page);
                view.continuation = continuation;
            }
        } else {
            let (selected, visible_range) = self
                .view
                .as_mut()
                .filter(|view| view.location.identity() == parent.identity())
                .map(|view| {
                    (
                        std::mem::take(&mut view.selected),
                        view.visible_range.clone(),
                    )
                })
                .unwrap_or_default();
            self.view = Some(DirectoryView {
                location: parent,
                pipeline: pending.pipeline.clone(),
                entries: entries.collect(),
                pages: vec![page],
                visible_range,
                continuation,
                selected,
            });
        }
        self.pending = None;
        if let Some(view) = &mut self.view {
            let start = view.visible_range.start.min(view.entries.len());
            view.visible_range = start..view.visible_range.end.min(view.entries.len());
            if view.pages.last().is_some_and(|page| page.complete) && !view.selected.is_empty() {
                // A missing selection may arrive on a later refresh page.
                let loaded: HashSet<_> = view
                    .entries
                    .iter()
                    .map(|entry| entry.location.identity())
                    .collect();
                view.selected.retain(|identity| loaded.contains(identity));
            }
        }
        true
    }
}
