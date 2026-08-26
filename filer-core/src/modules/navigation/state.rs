//! # Navigation State
//!
//! This module stores the per-session navigation snapshot, history, pipeline,
//! and selection. It keeps only reconstructable `LocationRef` values so state
//! remains valid as registry-backed compatibility state is removed.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//! use filer_core::modules::navigation::navigator::NavigatorState;
//!
//! let mut state = NavigatorState::new();
//! state.navigate_location(LocationRef::from_location(&Location::local("/tmp")));
//! assert!(state.current.is_some());
//! ```

use std::collections::VecDeque;

use rapidhash::RapidHashMap;
use serde::{Deserialize, Serialize};

use crate::model::location::{LocationId, LocationRef, LocationRoute};
use crate::pipeline::{Pipeline, PipelineConfig};

/// Navigation state sent to clients after a state change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavState {
    /// The current provider-aware location.
    #[serde(default)]
    pub current: Option<LocationRef>,
    /// Whether history contains a previous location.
    pub can_back: bool,
    /// Whether history contains a forward location.
    pub can_forward: bool,
    /// Whether the current direct-local location has a parent.
    pub can_up: bool,
    /// Current pipeline configuration.
    pub pipeline: PipelineConfig,
    /// Selected provider-aware locations.
    pub selected: Vec<LocationRef>,
}

impl Default for NavState {
    fn default() -> Self {
        Self {
            current: None,
            can_back: false,
            can_forward: false,
            can_up: false,
            pipeline: PipelineConfig::with_default_sort(),
            selected: Vec::new(),
        }
    }
}

/// A single provider-aware history item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationEntry {
    pub(crate) location: LocationRef,
}

impl NavigationEntry {
    fn new(location: LocationRef) -> Self {
        Self { location }
    }
}

/// Per-session navigation state.
#[derive(Debug)]
pub struct NavigatorState {
    /// Current provider-aware location.
    pub current: Option<LocationRef>,
    /// Navigation history stores complete provider-aware locations.
    pub history: VecDeque<NavigationEntry>,
    /// Current position in history, where zero means the newest entry.
    pub history_index: usize,
    /// Maximum number of history entries.
    pub history_limit: usize,
    /// Pipeline configuration.
    pub pipeline_config: PipelineConfig,
    /// Selection keyed by stable location identity.
    pub selected: RapidHashMap<LocationId, LocationRef>,
}

impl NavigatorState {
    /// Create a state with the default history limit.
    pub fn new() -> Self {
        Self::with_history_limit(100)
    }

    /// Create a state with a custom history limit.
    pub fn with_history_limit(limit: usize) -> Self {
        let mut history = VecDeque::new();
        history.reserve_exact(limit);
        Self {
            current: None,
            history,
            history_index: 0,
            history_limit: limit,
            pipeline_config: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            selected: RapidHashMap::default(),
        }
    }

    /// Build an executable pipeline from the current configuration.
    pub fn build_pipeline(&self) -> Pipeline {
        Pipeline::from_config(&self.pipeline_config)
    }

    /// Add a location to the navigation history.
    pub fn navigate_location(&mut self, location: LocationRef) {
        debug_assert!(self.history.len() >= self.history_index);
        if self.history_index != 0 {
            while self.history_index != 0 {
                self.history_index -= 1;
                self.history.pop_back();
            }
        }
        if self.history.len() == self.history_limit {
            self.history.pop_front();
        }
        self.history
            .push_back(NavigationEntry::new(location.clone()));
        self.current = Some(location);
    }

    /// Return the current location, if any.
    pub fn current_location(&self) -> Option<&LocationRef> {
        self.current.as_ref()
    }

    /// Move backward by `count` history entries.
    pub fn back(&mut self, count: usize) -> Option<LocationRef> {
        if count + self.history_index + 1 > self.history.len() || self.history.is_empty() {
            return None;
        }
        self.history_index += count;
        self.restore_history_entry()
    }

    /// Move forward by one history entry.
    pub fn forward(&mut self) -> Option<LocationRef> {
        if self.history_index == 0 {
            return None;
        }
        self.history_index -= 1;
        self.restore_history_entry()
    }

    /// Check whether a previous history entry exists.
    pub fn can_back(&self) -> bool {
        self.history.len() > self.history_index + 1
    }

    /// Check whether a forward history entry exists.
    pub fn can_forward(&self) -> bool {
        self.history_index != 0
    }

    /// Build the serializable state snapshot.
    pub fn snapshot(&self) -> NavState {
        let mut selected = self.selected.values().cloned().collect::<Vec<_>>();
        selected.sort_by_key(|location| location.identity().0);
        NavState {
            current: self.current.clone(),
            can_back: self.can_back(),
            can_forward: self.can_forward(),
            can_up: self.current.as_ref().is_some_and(has_parent),
            pipeline: self.pipeline_config.clone(),
            selected,
        }
    }

    fn restore_history_entry(&mut self) -> Option<LocationRef> {
        let location = self
            .history
            .get(self.history.len() - self.history_index - 1)
            .map(|entry| entry.location.clone())?;
        self.current = Some(location.clone());
        Some(location)
    }
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self::new()
    }
}

fn has_parent(location: &LocationRef) -> bool {
    let Some(descriptor) = location.descriptor() else {
        return false;
    };
    match descriptor.route() {
        LocationRoute::DirectPath { path } => path.parent().is_some(),
        LocationRoute::Segmented { .. } | LocationRoute::UnsupportedProvider { .. } => false,
    }
}
