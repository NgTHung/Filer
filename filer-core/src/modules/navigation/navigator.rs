//! Navigator actor - manages navigation state and history per session
//!
//! The Navigator is responsible for:
//! - Tracking current directory per session
//! - Managing back/forward history
//! - Coordinating with Scanner for directory listing
//! - Maintaining view settings (sort, filter, show hidden)

use std::collections::VecDeque;

use rapidhash::{RapidHashSet, fast::RandomState};
use std::sync::Arc;

use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::actors::Actor;
use crate::api::events;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationRef, LocationRoute};
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::scan::scanner::ScanCommand;
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::utils::channel::send_or_warn;
use crate::{CoreError, Event};

/// Navigation commands
#[derive(Debug, Clone)]
pub enum NavCommand {
    /// Navigate to a node (must be directory)
    Navigate {
        session: SessionId,
        node: NodeId,
        request: RequestId,
    },
    /// Navigate to path (for address bar input)
    NavigateToPath {
        session: SessionId,
        path: std::path::PathBuf,
        request: RequestId,
    },
    NavigateToLocation {
        session: SessionId,
        location: LocationRef,
        request: RequestId,
    },
    /// Go back in history
    Back(SessionId, RequestId),
    /// Go forward in history
    Forward(SessionId, RequestId),
    /// Go to parent directory
    Up(SessionId, RequestId),
    /// Refresh current directory
    Refresh(SessionId, RequestId),
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
    /// Remove session state (cleanup on DestroySession)
    RemoveSession(SessionId),
}

/// Navigation state snapshot (sent to UI via events)
///
/// This struct is serializable and sent over the wire to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavState {
    /// Compatibility/cache handle for the current direct-local directory.
    pub current: Option<NodeId>,
    /// Current provider-aware location, when known.
    #[serde(default)]
    pub current_location: Option<LocationRef>,
    /// Can navigate back
    pub can_back: bool,
    /// Can navigate forward
    pub can_forward: bool,
    /// Can navigate up (has parent)
    pub can_up: bool,
    /// Current pipeline configuration
    pub pipeline: PipelineConfig,
    /// Selection is still represented by compatibility/cache `NodeId` handles.
    pub selected: Vec<NodeId>,
}

impl Default for NavState {
    fn default() -> Self {
        Self {
            current: None,
            current_location: None,
            can_back: false,
            can_forward: false,
            can_up: false,
            pipeline: PipelineConfig::with_default_sort(),
            selected: Vec::new(),
        }
    }
}

/// A single history item with provider-aware identity and optional compat data.
#[derive(Debug)]
pub struct NavigationEntry {
    location: Option<LocationRef>,
    compat_node: Option<NodeId>,
}

impl NavigationEntry {
    fn new(location: Option<LocationRef>, compat_node: Option<NodeId>) -> Self {
        Self {
            location,
            compat_node,
        }
    }
}

impl PartialEq<NodeId> for NavigationEntry {
    fn eq(&self, other: &NodeId) -> bool {
        self.compat_node == Some(*other)
    }
}

/// Per-session navigator state
#[derive(Debug)]
pub struct NavigatorState {
    /// Compatibility/cache handle for the current direct-local directory.
    pub current: Option<NodeId>,
    /// Current provider-aware location, when known.
    pub current_location: Option<LocationRef>,
    /// Navigation history stores provider-aware locations first.
    pub history: VecDeque<NavigationEntry>,
    /// Current position in history (for back/forward)
    pub history_index: usize,
    /// Maximum history entries
    pub history_limit: usize,
    /// Pipeline configuration (serializable)
    pub pipeline_config: PipelineConfig,
    /// Selection stored as compatibility/cache `NodeId` handles.
    pub selected: RapidHashSet<NodeId>,

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
            current_location: None,
            history: his,
            history_index: 0,
            pipeline_config: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            selected: RapidHashSet::default(),
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
            current_location: None,
            history: hs,
            history_index: 0,
            pipeline_config: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            selected: RapidHashSet::default(),
        }
    }

    /// Build executable Pipeline from current config
    pub fn build_pipeline(&self) -> Pipeline {
        Pipeline::from_config(&self.pipeline_config)
    }

    /// Navigate to a new directory
    pub fn navigate(&mut self, node: NodeId) {
        let location = self.register.resolve_node_location(node);
        self.navigate_entry(location, Some(node));
    }

    /// Navigate to a new directory with a known provider-aware location.
    pub fn navigate_with_location(&mut self, node: NodeId, location: Option<LocationRef>) {
        self.navigate_entry(location, Some(node));
    }

    pub fn navigate_location(&mut self, location: LocationRef, compat_node: Option<NodeId>) {
        self.navigate_entry(Some(location), compat_node);
    }

    pub fn current_location(&self) -> Option<&LocationRef> {
        self.current_location.as_ref()
    }

    pub fn current_compat_node(&self) -> Option<NodeId> {
        self.current
    }

    fn navigate_entry(&mut self, location: Option<LocationRef>, compat_node: Option<NodeId>) {
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
            .push_back(NavigationEntry::new(location.clone(), compat_node));
        self.current = compat_node;
        self.current_location = location;
    }

    /// Go back in history
    pub fn back(&mut self, nums: usize) -> Option<NodeId> {
        if nums + self.history_index + 1 > self.history.len() {
            None
        } else if !self.history.is_empty() {
            self.history_index += nums;
            if let Some(entry) = self
                .history
                .get(self.history.len() - self.history_index - 1)
            {
                self.current = entry.compat_node;
                self.current_location = entry.location.clone();
            }
            self.current
        } else {
            None
        }
    }

    /// Go forward in history
    pub fn forward(&mut self) -> Option<NodeId> {
        if self.history_index != 0 {
            self.history_index -= 1;
            if let Some(entry) = self
                .history
                .get(self.history.len() - self.history_index - 1)
            {
                self.current = entry.compat_node;
                self.current_location = entry.location.clone();
            }
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
            current_location: self.current_location.clone(),
            can_back: self.can_back(),
            can_forward: self.can_forward(),
            can_up: self
                .current
                .and_then(|f| self.register.clone().have_par(f))
                .unwrap_or(false),
            pipeline: self.pipeline_config.clone(),
            selected: self.selected.iter().cloned().collect(),
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
    scanner_tx: Sender<ScanCommand>,
    sessions: Arc<scc::HashMap<SessionId, NavigatorState, RandomState>>,
    path_cache: Arc<scc::HashSet<NodeId, RandomState>>,
    register: NodeRegistry,
}

impl Navigator {
    pub fn new(
        commands: Receiver<NavCommand>,
        events: Sender<events::Event>,
        scanner_tx: Sender<ScanCommand>,
        reg: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            scanner_tx,
            sessions: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            path_cache: Arc::new(scc::HashSet::with_hasher(RandomState::new())),
            register: reg,
        }
    }

    /// Get or lazily create a session's navigator state.
    ///
    /// The Router already validates session existence via SessionManager
    /// before dispatching to us, so by the time a command arrives the
    /// session is known-valid.  We lazily create `NavigatorState` on
    /// first access, eliminating the need for a separate `NewSession`
    /// setup message and removing coupling with the Router lifecycle.
    async fn get_or_init(&self, session: SessionId) {
        if !self.sessions.contains_async(&session).await {
            let _ = self
                .sessions
                .insert_async(session, NavigatorState::new(self.register.clone()))
                .await;
        }
    }

    /// Emit a `CurrentNavigateState` snapshot to the UI.
    ///
    /// Called after every state-mutating command so the UI can
    /// immediately update breadcrumbs, back/forward buttons, and
    /// selection — before the async scan finishes.
    fn emit_snapshot(&self, session: SessionId) {
        self.sessions.read_sync(&session, |_, v| {
            send_or_warn(
                &self.events,
                Event::CurrentNavigateState {
                    session,
                    state: v.snapshot(),
                },
                "emit nav snapshot",
            );
        });
    }

    /// Handle a navigation command
    async fn handle_command(&self, cmd: NavCommand) {
        match cmd {
            NavCommand::Navigate {
                session,
                node,
                request,
            } => {
                self.get_or_init(session).await;
                let Some(location) = self.register.resolve_node_location(node) else {
                    send_or_warn(
                        &self.events,
                        Event::from_request_error(
                            CoreError::navigation_unavailable(format!(
                                "Can't navigate: unresolved node {node:?}"
                            )),
                            session,
                            request,
                        ),
                        "navigate node resolve",
                    );
                    return;
                };
                let _ = self.path_cache.insert_async(node).await;
                self.sessions
                    .update_async(&session, |_, v| {
                        v.navigate_location(location.clone(), Some(node));
                        Self::trigger_compat_scan(
                            session,
                            location.clone(),
                            v,
                            self.scanner_tx.clone(),
                            request,
                        );
                    })
                    .await;
                self.emit_snapshot(session);
            }
            NavCommand::NavigateToPath {
                session,
                path,
                request,
            } => {
                self.get_or_init(session).await;
                let node = self.register.clone().register(path.clone());
                let location = LocationRef::from_location(&Location::local(path));
                let _ = self.path_cache.insert_async(node).await;
                self.sessions
                    .update_async(&session, |_, v| {
                        v.navigate_location(location.clone(), Some(node));
                        Self::trigger_compat_scan(
                            session,
                            location.clone(),
                            v,
                            self.scanner_tx.clone(),
                            request,
                        );
                    })
                    .await;
                self.emit_snapshot(session);
            }
            NavCommand::NavigateToLocation {
                session,
                location,
                request,
            } => {
                self.get_or_init(session).await;
                let location = match self.register.resolve_location_ref(&location) {
                    Ok(location) => location,
                    Err(error) => {
                        send_or_warn(
                            &self.events,
                            Event::from_request_error(error, session, request),
                            "navigate resolve",
                        );
                        return;
                    }
                };
                let route = location.route();
                let node = match &route {
                    LocationRoute::DirectPath { .. } => {
                        match self.register.register_location_node(location.clone()) {
                            Ok(node) => node,
                            Err(error) => {
                                send_or_warn(
                                    &self.events,
                                    Event::from_request_error(error, session, request),
                                    "navigate register",
                                );
                                return;
                            }
                        }
                    }
                    LocationRoute::Segmented { .. } => self
                        .register
                        .register_segmented_location_node(location.clone()),
                    LocationRoute::UnsupportedProvider { .. } => {
                        let error = route.require_direct_path().unwrap_err();
                        send_or_warn(
                            &self.events,
                            Event::from_request_error(error, session, request),
                            "navigate route",
                        );
                        return;
                    }
                };
                let _ = self.path_cache.insert_async(node).await;
                self.sessions
                    .update_async(&session, |_, v| {
                        v.navigate_with_location(node, Some(LocationRef::from_location(&location)));
                        send_or_warn(
                            &self.scanner_tx,
                            ScanCommand::ScanLocation {
                                location: LocationRef::from_location(&location),
                                session,
                                pipeline: v.pipeline_config.clone(),
                                load: DirectoryLoadOptions::default(),
                                request,
                            },
                            "trigger location scan",
                        );
                    })
                    .await;
                if let LocationRoute::DirectPath { path } = route {
                    self.register.clone().register(path);
                }
                self.emit_snapshot(session);
            }
            NavCommand::Back(session_id, request) => {
                self.get_or_init(session_id).await;
                self.sessions
                    .update_async(&session_id, |_, v| {
                        if v.can_back() {
                            let _ = v.back(1);
                            Self::trigger_current_scan(
                                session_id,
                                v,
                                self.scanner_tx.clone(),
                                request,
                            );
                        } else {
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(
                                    CoreError::navigation_unavailable("Can't go back: no history"),
                                    session_id,
                                    request,
                                ),
                                "emit back error",
                            );
                        }
                    })
                    .await;
                self.emit_snapshot(session_id);
            }
            NavCommand::Forward(session_id, request) => {
                self.get_or_init(session_id).await;
                self.sessions
                    .update_async(&session_id, |_, v| {
                        if v.can_forward() {
                            let _ = v.forward();
                            Self::trigger_current_scan(
                                session_id,
                                v,
                                self.scanner_tx.clone(),
                                request,
                            );
                        } else {
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(
                                    CoreError::navigation_unavailable(
                                        "Can't go forward: no forward history",
                                    ),
                                    session_id,
                                    request,
                                ),
                                "emit forward error",
                            );
                        }
                    })
                    .await;
                self.emit_snapshot(session_id);
            }
            NavCommand::Up(session_id, request) => {
                self.get_or_init(session_id).await;
                self.sessions
                    .update_async(&session_id, |_, v| {
                        if let Some((location, node)) = Self::parent_entry(v, &self.register) {
                            v.navigate_location(location, node);
                            Self::trigger_current_scan(
                                session_id,
                                v,
                                self.scanner_tx.clone(),
                                request,
                            );
                        } else {
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(
                                    CoreError::navigation_unavailable(
                                        "Can't go up: no parent directory",
                                    ),
                                    session_id,
                                    request,
                                ),
                                "emit up error",
                            );
                        }
                    })
                    .await;
                self.emit_snapshot(session_id);
            }
            NavCommand::Refresh(session_id, request) => {
                self.get_or_init(session_id).await;
                self.sessions
                    .read_async(&session_id, |_k, v| {
                        if v.current.is_some() || v.current_location.is_some() {
                            Self::trigger_current_refresh_scan(
                                session_id,
                                v,
                                self.scanner_tx.clone(),
                                request,
                            );
                        } else {
                            send_or_warn(
                                &self.events,
                                Event::from_request_error(
                                    CoreError::navigation_unavailable(
                                        "Can't refresh: no current directory",
                                    ),
                                    session_id,
                                    request,
                                ),
                                "emit refresh error",
                            );
                        }
                    })
                    .await;
                // Refresh doesn't change state, so no snapshot needed
            }
            NavCommand::SetPipeline { session, config } => {
                self.get_or_init(session).await;
                self.sessions
                    .update_async(&session, |_k, v| {
                        v.pipeline_config = config;
                    })
                    .await;
                self.emit_snapshot(session);
            }
            NavCommand::SetSelected { session, nodes } => {
                self.get_or_init(session).await;
                self.sessions
                    .update_async(&session, |_k, v: &mut NavigatorState| {
                        v.selected.extend(nodes.iter());
                    })
                    .await;
                self.emit_snapshot(session);
            }
            NavCommand::GetState(session_id) => {
                self.get_or_init(session_id).await;
                self.sessions
                    .read_async(&session_id, |_k, v| {
                        self.events.send(Event::CurrentNavigateState {
                            session: session_id,
                            state: v.snapshot(),
                        })
                    })
                    .await;
            }
            NavCommand::Invalidate(node_id) => {
                if self.path_cache.contains_async(&node_id).await {
                    self.sessions
                        .iter_async(|k, v| {
                            if v.current == Some(node_id) {
                                Self::trigger_current_refresh_scan(
                                    *k,
                                    v,
                                    self.scanner_tx.clone(),
                                    RequestId::new(),
                                );
                            }
                            true
                        })
                        .await;
                }
            }
            NavCommand::RemoveSession(session_id) => {
                let _ = self.sessions.remove_async(&session_id).await;
            }
        }
    }

    fn parent_entry(
        state: &NavigatorState,
        registry: &NodeRegistry,
    ) -> Option<(LocationRef, Option<NodeId>)> {
        if let Some(location_ref) = state.current_location() {
            let location = registry.resolve_location_ref(location_ref).ok()?;
            if let LocationRoute::DirectPath { path } = location.route() {
                let parent = path.parent()?.to_path_buf();
                let parent_location = Location::local(parent);
                let node = registry
                    .register_location_node(parent_location.clone())
                    .ok();
                return Some((LocationRef::from_location(&parent_location), node));
            }
        }

        let parent = state
            .current_compat_node()
            .and_then(|node| registry.clone().get_par(node))?;
        let parent_location = Location::local(parent);
        let node = registry
            .register_location_node(parent_location.clone())
            .ok();
        Some((LocationRef::from_location(&parent_location), node))
    }

    fn trigger_compat_scan(
        session: SessionId,
        location: LocationRef,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        send_or_warn(
            &scanner_tx,
            ScanCommand::ScanCompat {
                location,
                session,
                pipeline: state.pipeline_config.clone(),
                load: DirectoryLoadOptions::default(),
                request,
            },
            "trigger scan",
        );
    }

    /// Trigger a scan for the current directory, preferring the Location-native
    /// route when navigation state has one.
    fn trigger_current_scan(
        session: SessionId,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        if let Some(location) = state.current_location.clone() {
            send_or_warn(
                &scanner_tx,
                ScanCommand::ScanLocation {
                    location,
                    session,
                    pipeline: state.pipeline_config.clone(),
                    load: DirectoryLoadOptions::default(),
                    request,
                },
                "trigger location scan",
            );
        } else if let Some(node) = state.current {
            if let Some(location) = state.register.resolve_node_location(node) {
                Self::trigger_compat_scan(session, location, state, scanner_tx, request);
            }
        }
    }

    /// Trigger a fresh scan of the current directory.
    ///
    /// Refresh is user- or watcher-driven and must bypass the directory cache;
    /// otherwise filesystem changes can be detected but hidden by stale cached
    /// listings.
    fn trigger_refresh_scan(
        session: SessionId,
        node: NodeId,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        let location = state.register.resolve_node_location(node).or_else(|| {
            state
                .register
                .resolve(node)
                .map(|path| LocationRef::from_location(&Location::local(path)))
        });
        let Some(location) = location else {
            return;
        };
        send_or_warn(
            &scanner_tx,
            ScanCommand::RefreshCompat {
                location,
                session,
                pipeline: state.pipeline_config.clone(),
                load: DirectoryLoadOptions::default(),
                request,
            },
            "trigger refresh scan",
        );
    }

    /// Trigger a fresh scan of the current directory, preferring Location-native
    /// cache-bypass semantics when navigation state has a Location.
    fn trigger_current_refresh_scan(
        session: SessionId,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        if let Some(location) = state.current_location.clone() {
            send_or_warn(
                &scanner_tx,
                ScanCommand::RefreshLocation {
                    location,
                    session,
                    pipeline: state.pipeline_config.clone(),
                    load: DirectoryLoadOptions::default(),
                    request,
                },
                "trigger location refresh scan",
            );
        } else if let Some(node) = state.current {
            Self::trigger_refresh_scan(session, node, state, scanner_tx, request);
        }
    }
}

impl Actor for Navigator {
    async fn run(self) {
        while let Ok(command) = self.commands.recv_async().await {
            self.handle_command(command).await;
        }
    }

    fn name(&self) -> &'static str {
        "navigator"
    }
}
