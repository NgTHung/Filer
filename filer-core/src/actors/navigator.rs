//! Navigator actor - manages navigation state and history per session
//!
//! The Navigator is responsible for:
//! - Tracking current directory per session
//! - Managing back/forward history
//! - Coordinating with Scanner for directory listing
//! - Maintaining view settings (sort, filter, show hidden)

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::Event;
use crate::actors::{Actor, scanner};
use crate::api::events;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig};
use scanner::ScanCommand;

/// Navigation commands
#[derive(Debug, Clone)]
pub enum NavCommand {
    /// Navigate to a node (must be directory)
    Navigate {
        session: SessionId,
        node: NodeId,
    },
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
    Invalidate(NodeId),
    NewSession(SessionId)
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
    pub history: VecDeque<NodeId>,
    /// Current position in history (for back/forward)
    pub history_index: usize,
    /// Maximum history entries
    pub history_limit: usize,
    /// Pipeline configuration (serializable)
    pub pipeline_config: PipelineConfig,
    /// Selected nodes
    pub selected: HashSet<NodeId>,

    pub register: NodeRegistry,
}

impl NavigatorState {
    pub fn new(reg: NodeRegistry) -> Self {
        let mut his = VecDeque::new();
        his.reserve_exact(100);
        Self {
            history_limit: 100,
            register: reg,
            current: None,
            history: his,
            history_index: 0,
            pipeline_config: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            selected: HashSet::new(),
        }
    }

    /// Create new navigator state with custom history limit
    pub fn with_history_limit(limit: usize, reg: NodeRegistry) -> Self {
        let mut hs = VecDeque::new();
        hs.reserve_exact(limit);
        Self {
            history_limit: limit,
            register: reg,
            current: None,
            history: hs,
            history_index: 0,
            pipeline_config: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            selected: HashSet::new(),
        }
    }

    /// Build executable Pipeline from current config
    pub fn build_pipeline(&self) -> Pipeline {
        Pipeline::from_config(&self.pipeline_config)
    }

    /// Navigate to a new directory
    pub fn navigate(&mut self, node: NodeId) {
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
        self.history.push_back(node);
        self.current = Some(node);
    }

    /// Go back in history
    pub fn back(&mut self, nums: usize) -> Option<NodeId> {
        if nums + self.history_index + 1 > self.history.len() {
            None
        } else if self.history.len() != 0 {
            self.history_index += nums;
            self.current = self.history
                .get(self.history.len() - self.history_index - 1)
                .copied();
            self.current
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn forward(&mut self) -> Option<NodeId> {
        if self.history_index != 0 {
            self.history_index -= 1;
            self.current = self.history
                .get(self.history.len() - self.history_index - 1)
                .copied();
            self.current
        } else {
            None
        }
    }

    /// Check if can go back
    pub fn can_back(&self) -> bool {
        self.history.len() > self.history_index + 1
    }

    /// Check if can go forward
    pub fn can_forward(&self) -> bool {
        self.history_index != 0
    }

    /// Get current state snapshot
    pub fn snapshot(&self) -> NavState {
        NavState {
            current: self.current,
            can_back: self.can_back(),
            can_forward: self.can_forward(),
            can_up: self
                .current
                .map(|f| self.register.clone().have_par(f).is_some())
                .is_some(),
            pipeline: self.pipeline_config.clone(),
            selected: self.selected.iter().map(|f| f.clone()).collect(),
        }
    }
}

/// Navigator actor - coordinates navigation across sessions
pub struct Navigator {
    /// Incoming commands
    commands: Receiver<NavCommand>,
    /// Outgoing events
    events: Sender<events::Event>,
    /// Scanner channel for triggering scans
    scanner_tx: Sender<scanner::ScanCommand>,
    sessions: Arc<scc::HashMap<SessionId, NavigatorState>>,
    path_cache: Arc<scc::HashSet<NodeId>>,
    register: NodeRegistry,
}

impl Navigator {
    pub fn new(
        commands: Receiver<NavCommand>,
        events: Sender<events::Event>,
        scanner_tx: Sender<scanner::ScanCommand>,
        reg: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            scanner_tx,
            sessions: Arc::new(scc::HashMap::new()),
            path_cache: Arc::new(scc::HashSet::new()),
            register: reg,
        }
    }

    /// Handle a navigation command
    async fn handle_command(
        cmd: NavCommand,
        sessions: Arc<scc::HashMap<SessionId, NavigatorState>>,
        register: NodeRegistry,
        path_cache: Arc<scc::HashSet<NodeId>>,
        scanner_tx: &Sender<scanner::ScanCommand>,
        events: &Sender<events::Event>,
    ) {
        match cmd {
            NavCommand::Navigate { session, node } => {
                sessions
                    .update_async(&session, |_, v| {
                        v.navigate(node);
                        Self::trigger_scan(session, node, v, scanner_tx.clone());
                    })
                    .await;
            }
            NavCommand::NavigateToPath { session, path } => {
                sessions
                    .update_async(&session, |_, v| {
                        v.navigate(register.clone().register(path.clone()));
                        Self::trigger_scan(
                            session,
                            register.clone().register(path),
                            v,
                            scanner_tx.clone(),
                        );
                    })
                    .await;
            }
            NavCommand::Back(session_id) => {
                sessions
                    .update_async(&session_id, |_, v| {
                        if v.can_back() {
                            let node = v.back(1).unwrap();
                            Self::trigger_scan(session_id, node, v, scanner_tx.clone());
                        } else {
                            let _ = events.send(Event::Error {
                                message: "Cant go back!".to_string(),
                                recoverable: true,
                                session: session_id,
                            });
                        }
                    })
                    .await;
            }
            NavCommand::Forward(session_id) => {
                sessions
                    .update_async(&session_id, |_, v| {
                        if v.can_forward() {
                            let node = v.forward().unwrap();
                            Self::trigger_scan(session_id, node, v, scanner_tx.clone());
                        } else {
                            let _ = events.send(Event::Error {
                                message: "Cant go forward!".to_string(),
                                recoverable: true,
                                session: session_id,
                            });
                        }
                    })
                    .await;
            }
            NavCommand::Up(session_id) => {
                sessions
                    .update_async(&session_id, |_, v| {
                        if v.current
                            .map(|f| register.clone().have_par(f).is_some())
                            .is_some()
                        {
                            let par = v
                                .current
                                .map(|f| register.clone().get_par(f))
                                .flatten()
                                .unwrap();
                            let node = register.clone().register(par);
                            v.navigate(node);
                            Self::trigger_scan(session_id, node, v, scanner_tx.clone());
                        } else {
                            let _ = events.send(Event::Error {
                                message: "Cant go up!".to_string(),
                                recoverable: true,
                                session: session_id,
                            });
                        }
                    })
                    .await;
            }
            NavCommand::Refresh(session_id) => {
                sessions
                    .read_async(&session_id, |_k, v| {
                        if v.current.is_some() {
                            let cur = v.current.unwrap();
                            Self::trigger_scan(session_id, cur, v, scanner_tx.clone());
                        } else {
                            let _ = events.send(Event::Error {
                                message: "Cant refresh!".to_string(),
                                recoverable: true,
                                session: session_id,
                            });
                        }
                    })
                    .await;
            }
            NavCommand::SetPipeline { session, config } => {
                sessions
                    .update_async(&session, |_k, v| {
                        v.pipeline_config = config;
                    })
                    .await;
            }
            NavCommand::SetSelected { session, nodes } => {
                sessions
                    .update_async(&session, |_k, v: &mut NavigatorState| {
                        v.selected.extend(nodes.iter());
                    })
                    .await;
            }
            NavCommand::GetState(session_id) => {
                sessions
                    .read_async(&session_id, |_k, v| {
                        events.send(Event::CurrentNavigateState {
                            session: session_id,
                            state: v.snapshot(),
                        })
                    })
                    .await;
            }
            NavCommand::Invalidate(node_id) => {
                if path_cache.contains_async(&node_id).await {
                    sessions
                        .iter_async(|k, v| {
                            let verd = v.current.map(|c| c == node_id);
                            if verd.is_some() && verd.unwrap() == true {
                                Self::trigger_scan(k.clone(), node_id, v, scanner_tx.clone());
                            }
                            true
                        })
                        .await;
                }
            }
            NavCommand::NewSession(session_id) => {
                let _ = sessions.insert_async(session_id, NavigatorState::new(register.clone())).await;
            },
        }
    }

    /// Trigger a scan of the current directory
    fn trigger_scan(
        session: SessionId,
        node: NodeId,
        state: &NavigatorState,
        scanner_tx: Sender<crate::actors::scanner::ScanCommand>,
    ) {
        let _ = scanner_tx.send(ScanCommand::ScanNode {
            node,
            session,
            pipeline: state.pipeline_config.clone(),
        });
    }

    fn handler(
        cmd: NavCommand,
        sessions: Arc<scc::HashMap<SessionId, NavigatorState>>,
        register: NodeRegistry,
        path_cache: Arc<scc::HashSet<NodeId>>,
        scanner_tx: Sender<scanner::ScanCommand>,
        events: Sender<events::Event>,
    ) {
        tokio::spawn(async move {
            Self::handle_command(
                cmd,
                sessions.clone(),
                register.clone(),
                path_cache.clone(),
                &scanner_tx,
                &events,
            )
            .await;
        });
    }
}

impl Actor for Navigator {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(command) => {
                    Self::handler(
                        command,
                        self.sessions.clone(),
                        self.register.clone(),
                        self.path_cache.clone(),
                        self.scanner_tx.clone(),
                        self.events.clone(),
                    );
                }
                Err(_) => {
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "navigator"
    }
}
