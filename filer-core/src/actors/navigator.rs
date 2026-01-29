//! Navigator actor - manages navigation state and history per session
//!
//! The Navigator is responsible for:
//! - Tracking current directory per session
//! - Managing back/forward history
//! - Coordinating with Scanner for directory listing
//! - Maintaining view settings (sort, filter, show hidden)

use std::collections::HashSet;

use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::model::node::NodeId;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig, SortConfig, SortField, SortOrder};

/// Navigation commands
#[derive(Debug, Clone)]
pub enum NavCommand {
    /// Navigate to a node (must be directory)
    Navigate { session: SessionId, node: NodeId },
    /// Navigate to path (for address bar input)
    NavigateToPath {
        session: SessionId,
        path: std::path::PathBuf,
    },
    /// Go back in history
    Back(SessionId),
    /// Go forward in history
    Forward(SessionId),
    /// Go to parent directory
    Up(SessionId),
    /// Refresh current directory
    Refresh(SessionId),
    /// Update entire pipeline config (sort, filter, group)
    SetPipeline {
        session: SessionId,
        config: PipelineConfig,
    },
    /// Update selection
    SetSelected {
        session: SessionId,
        nodes: Vec<NodeId>,
    },
    /// Get current state snapshot
    GetState(SessionId),
}

/// Navigation state snapshot (sent to UI via events)
///
/// This struct is serializable and sent over the wire to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavState {
    /// Current directory NodeId
    pub current: Option<NodeId>,
    /// Can navigate back
    pub can_back: bool,
    /// Can navigate forward
    pub can_forward: bool,
    /// Can navigate up (has parent)
    pub can_up: bool,
    /// Current pipeline configuration
    pub pipeline: PipelineConfig,
    /// Currently selected nodes
    pub selected: Vec<NodeId>,
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

/// Per-session navigator state
#[derive(Debug)]
pub struct NavigatorState {
    /// Current directory
    pub current: Option<NodeId>,
    /// Navigation history (directories visited)
    pub history: Vec<NodeId>,
    /// Current position in history (for back/forward)
    pub history_index: usize,
    /// Maximum history entries
    pub history_limit: usize,
    /// Pipeline configuration (serializable)
    pub pipeline_config: PipelineConfig,
    /// Selected nodes
    pub selected: HashSet<NodeId>,
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self {
            current: None,
            history: Vec::new(),
            history_index: 0,
            history_limit: 100,
            pipeline_config: PipelineConfig::with_default_sort(),
            selected: HashSet::new(),
        }
    }
}

impl NavigatorState {
    /// Create new navigator state with custom history limit
    pub fn with_history_limit(limit: usize) -> Self {
        Self {
            history_limit: limit,
            ..Default::default()
        }
    }

    /// Build executable Pipeline from current config
    pub fn build_pipeline(&self) -> Pipeline {
        Pipeline::from_config(&self.pipeline_config)
    }

    /// Navigate to a new directory
    pub fn navigate(&mut self, node: NodeId) {
        todo!()
    }

    /// Go back in history
    pub fn back(&mut self) -> Option<NodeId> {
        todo!()
    }

    /// Go forward in history
    pub fn forward(&mut self) -> Option<NodeId> {
        todo!()
    }

    /// Check if can go back
    pub fn can_back(&self) -> bool {
        todo!()
    }

    /// Check if can go forward
    pub fn can_forward(&self) -> bool {
        todo!()
    }

    /// Get current state snapshot
    pub fn snapshot(&self) -> NavState {
        todo!()
    }
}

/// Navigator actor - coordinates navigation across sessions
pub struct Navigator {
    /// Incoming commands
    commands: Receiver<NavCommand>,
    /// Outgoing events
    events: Sender<crate::api::events::Event>,
    /// Scanner channel for triggering scans
    scanner_tx: Sender<crate::actors::scanner::ScanCommand>,
}

impl Navigator {
    pub fn new(
        commands: Receiver<NavCommand>,
        events: Sender<crate::api::events::Event>,
        scanner_tx: Sender<crate::actors::scanner::ScanCommand>,
    ) -> Self {
        todo!()
    }

    /// Handle a navigation command
    async fn handle_command(&mut self, cmd: NavCommand) {
        todo!()
    }

    /// Trigger a scan of the current directory
    fn trigger_scan(&self, session: SessionId, node: NodeId, state: &NavigatorState) {
        todo!()
    }
}

impl crate::actors::Actor for Navigator {
    async fn run(mut self) {
        todo!()
    }

    fn name(&self) -> &'static str {
        "navigator"
    }
}
