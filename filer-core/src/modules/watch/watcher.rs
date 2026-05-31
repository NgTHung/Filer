use flume::Sender;
use std::path::PathBuf;

use rapidhash::RapidHashMap;
use std::sync::Arc;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::navigation::navigator::NavCommand;
use crate::utils::channel::send_or_warn;
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};

/// Commands for watcher actor
#[derive(Debug, Clone)]
pub enum WatchCommand {
    Watch(NodeId, SessionId),
    WatchLocation {
        location: LocationRef,
        session: SessionId,
        request: RequestId,
    },
    Unwatch(NodeId),
    UnwatchLocation {
        location: LocationRef,
        session: SessionId,
    },
    UnwatchSession(SessionId),
    UnwatchAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum WatchKey {
    Node(NodeId),
    Location(crate::model::location::LocationId),
}

/// Tracks which sessions are watching which paths
struct WatchEntry {
    path: PathBuf,
    node: Option<NodeId>,
    location: Option<LocationRef>,
    sessions: Vec<SessionId>,
    #[allow(dead_code)]
    handle: Box<dyn WatchHandle>,
}

/// Watcher actor - monitors filesystem changes via a pluggable [`WatchProvider`].
pub struct Watcher {
    commands: flume::Receiver<WatchCommand>,
    events: Sender<Event>,
    registry: NodeRegistry,
    watches: RapidHashMap<WatchKey, WatchEntry>,
    provider: Arc<dyn WatchProvider>,
    /// Receives raw changes from the provider; forwarded as Events.
    change_rx: flume::Receiver<FsChange>,
    change_tx: flume::Sender<FsChange>,
    refresh_tx: Option<Sender<NavCommand>>,
}

impl Watcher {
    pub fn new(
        commands: flume::Receiver<WatchCommand>,
        events: Sender<Event>,
        registry: NodeRegistry,
        provider: Arc<dyn WatchProvider>,
    ) -> Self {
        Self::new_inner(commands, events, registry, provider, None)
    }

    pub fn with_refresh(
        commands: flume::Receiver<WatchCommand>,
        events: Sender<Event>,
        registry: NodeRegistry,
        provider: Arc<dyn WatchProvider>,
        refresh_tx: Sender<NavCommand>,
    ) -> Self {
        Self::new_inner(commands, events, registry, provider, Some(refresh_tx))
    }

    fn new_inner(
        commands: flume::Receiver<WatchCommand>,
        events: Sender<Event>,
        registry: NodeRegistry,
        provider: Arc<dyn WatchProvider>,
        refresh_tx: Option<Sender<NavCommand>>,
    ) -> Self {
        let (change_tx, change_rx) = flume::unbounded();
        Self {
            commands,
            events,
            registry,
            watches: RapidHashMap::default(),
            provider,
            change_rx,
            change_tx,
            refresh_tx,
        }
    }

    /// Handle a watch command
    async fn handle_watch(&mut self, node_id: NodeId, session_id: SessionId) {
        // Resolve node to path
        let path = match self.registry.resolve(node_id) {
            Some(p) => p,
            None => {
                tracing::warn!("Cannot watch node {:?}: not found in registry", node_id);
                return;
            }
        };

        // If already watching, just add the session
        if let Some(entry) = self.watches.get_mut(&WatchKey::Node(node_id)) {
            if !entry.sessions.contains(&session_id) {
                entry.sessions.push(session_id);
                tracing::debug!(
                    node = ?node_id,
                    path = %entry.path.display(),
                    session = ?session_id,
                    "Added session to existing watch"
                );
            }
            return;
        }

        // Ask the provider to start watching
        match self.provider.watch(&path, self.change_tx.clone()).await {
            Ok(handle) => {
                let entry = WatchEntry {
                    path: path.clone(),
                    node: Some(node_id),
                    location: None,
                    sessions: vec![session_id],
                    handle,
                };
                self.watches.insert(WatchKey::Node(node_id), entry);
                tracing::debug!(
                    node = ?node_id,
                    path = %path.display(),
                    session = ?session_id,
                    "Started watching path"
                );
            }
            Err(e) => {
                tracing::error!(path = %path.display(), error = %e, "Failed to watch path");
            }
        }
    }

    async fn handle_watch_location(
        &mut self,
        location_ref: LocationRef,
        session_id: SessionId,
        request: RequestId,
    ) {
        let location = match self.registry.resolve_location_ref(&location_ref) {
            Ok(location) => location,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    Event::from_request_error(error, session_id, request),
                    "watch location resolve error",
                );
                return;
            }
        };

        let path = match location.route().require_direct_path() {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                send_or_warn(
                    &self.events,
                    Event::from_request_error(error, session_id, request),
                    "watch location route error",
                );
                return;
            }
        };

        let key = WatchKey::Location(location.id());
        let location_ref = LocationRef::from_location(&location);
        let node = self.registry.register_location_node(location).ok();

        if let Some(entry) = self.watches.get_mut(&key) {
            if !entry.sessions.contains(&session_id) {
                entry.sessions.push(session_id);
            }
            return;
        }

        match self.provider.watch(&path, self.change_tx.clone()).await {
            Ok(handle) => {
                self.watches.insert(
                    key,
                    WatchEntry {
                        path: path.clone(),
                        node,
                        location: Some(location_ref),
                        sessions: vec![session_id],
                        handle,
                    },
                );
            }
            Err(error) => {
                send_or_warn(
                    &self.events,
                    Event::from_request_error(error, session_id, request),
                    "watch location provider error",
                );
            }
        }
    }

    /// Handle an unwatch command
    async fn handle_unwatch(&mut self, node_id: NodeId) {
        if let Some(entry) = self.watches.remove(&WatchKey::Node(node_id)) {
            if let Err(e) = self.provider.unwatch(&entry.path).await {
                tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch path");
            } else {
                tracing::debug!(path = %entry.path.display(), "Stopped watching path");
            }
            // handle is dropped here, which also cleans up provider resources
        }
    }

    async fn handle_unwatch_location(&mut self, location_ref: LocationRef, session_id: SessionId) {
        let Ok(location) = self.registry.resolve_location_ref(&location_ref) else {
            tracing::warn!(session = ?session_id, "Cannot unwatch unresolved location");
            return;
        };

        let key = WatchKey::Location(location.id());
        if let Some(mut entry) = self.watches.remove(&key) {
            entry.sessions.retain(|s| *s != session_id);
            if entry.sessions.is_empty() {
                if let Err(e) = self.provider.unwatch(&entry.path).await {
                    tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch location");
                }
            } else {
                self.watches.insert(key, entry);
            }
        }
    }

    /// Handle unwatch session command
    async fn handle_unwatch_session(&mut self, session_id: SessionId) {
        let mut to_remove = Vec::new();

        for (key, entry) in &mut self.watches {
            entry.sessions.retain(|s| *s != session_id);
            if entry.sessions.is_empty() {
                to_remove.push(key.clone());
            }
        }

        for key in to_remove {
            if let Some(entry) = self.watches.remove(&key)
                && let Err(e) = self.provider.unwatch(&entry.path).await
            {
                tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch path for session cleanup");
            }
        }
    }

    /// Handle unwatch all command
    async fn handle_unwatch_all(&mut self) {
        let entries: Vec<_> = self.watches.drain().collect();
        for (_, entry) in entries {
            if let Err(e) = self.provider.unwatch(&entry.path).await {
                tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch path during shutdown");
            }
        }
    }

    /// Route a raw [`FsChange`] from the provider to the matching sessions.
    fn dispatch_change(&self, change: FsChange) {
        tracing::trace!(
            path = %change.path.display(),
            kind = ?change.kind,
            "Watcher received provider change"
        );
        for (_key, entry) in &self.watches {
            if change.path.starts_with(&entry.path) {
                for session in &entry.sessions {
                    tracing::debug!(
                        path = %change.path.display(),
                        kind = ?change.kind,
                        session = ?session,
                        "Watcher dispatching FsChangedCompat"
                    );
                    let evt = if let Some(location) = &entry.location {
                        Event::FsChanged {
                            location: location.clone(),
                            kind: change.kind.clone(),
                            session: *session,
                        }
                    } else {
                        Event::FsChangedCompat {
                            node: entry.node.expect("legacy watch entry must have node"),
                            kind: change.kind.clone(),
                            session: *session,
                        }
                    };
                    send_or_warn(&self.events, evt, "emit FsChangedCompat");
                }

                if let (Some(refresh_tx), Some(node)) = (&self.refresh_tx, entry.node) {
                    send_or_warn(
                        refresh_tx,
                        NavCommand::Invalidate(node),
                        "watch refresh invalidate",
                    );
                }
            }
        }
    }
}

impl Actor for Watcher {
    async fn run(mut self) {
        tracing::debug!("Watcher actor started");

        loop {
            tokio::select! {
                cmd = self.commands.recv_async() => {
                    match cmd {
                        Ok(WatchCommand::Watch(node, session)) => {
                            self.handle_watch(node, session).await;
                        }
                        Ok(WatchCommand::WatchLocation { location, session, request }) => {
                            self.handle_watch_location(location, session, request).await;
                        }
                        Ok(WatchCommand::Unwatch(node)) => {
                            self.handle_unwatch(node).await;
                        }
                        Ok(WatchCommand::UnwatchLocation { location, session }) => {
                            self.handle_unwatch_location(location, session).await;
                        }
                        Ok(WatchCommand::UnwatchSession(session)) => {
                            self.handle_unwatch_session(session).await;
                        }
                        Ok(WatchCommand::UnwatchAll) => {
                            self.handle_unwatch_all().await;
                        }
                        Err(_) => {
                            tracing::debug!("Watcher actor shutting down");
                            break;
                        }
                    }
                }
                change = self.change_rx.recv_async() => {
                    match change {
                        Ok(fs_change) => self.dispatch_change(fs_change),
                        Err(_) => {
                            // change channel closed — shouldn't happen while we hold change_tx
                            tracing::warn!("FsChange channel closed unexpectedly");
                        }
                    }
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "watcher"
    }
}
