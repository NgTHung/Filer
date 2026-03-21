use flume::Sender;
use std::path::PathBuf;

use rapidhash::RapidHashMap;
use std::sync::Arc;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::utils::channel::send_or_warn;
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};

/// Commands for watcher actor
#[derive(Debug, Clone)]
pub enum WatchCommand {
    Watch(NodeId, SessionId),
    Unwatch(NodeId),
    UnwatchSession(SessionId),
    UnwatchAll,
}

/// Tracks which sessions are watching which paths
struct WatchEntry {
    path: PathBuf,
    sessions: Vec<SessionId>,
    #[allow(dead_code)]
    handle: Box<dyn WatchHandle>,
}

/// Watcher actor - monitors filesystem changes via a pluggable [`WatchProvider`].
pub struct Watcher {
    commands: flume::Receiver<WatchCommand>,
    events: Sender<Event>,
    registry: NodeRegistry,
    watches: RapidHashMap<NodeId, WatchEntry>,
    provider: Arc<dyn WatchProvider>,
    /// Receives raw changes from the provider; forwarded as Events.
    change_rx: flume::Receiver<FsChange>,
    change_tx: flume::Sender<FsChange>,
}

impl Watcher {
    pub fn new(
        commands: flume::Receiver<WatchCommand>,
        events: Sender<Event>,
        registry: NodeRegistry,
        provider: Arc<dyn WatchProvider>,
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
        if let Some(entry) = self.watches.get_mut(&node_id) {
            if !entry.sessions.contains(&session_id) {
                entry.sessions.push(session_id);
            }
            return;
        }

        // Ask the provider to start watching
        match self.provider.watch(&path, self.change_tx.clone()).await {
            Ok(handle) => {
                let entry = WatchEntry {
                    path: path.clone(),
                    sessions: vec![session_id],
                    handle,
                };
                self.watches.insert(node_id, entry);
                tracing::debug!("Started watching {:?} for session {:?}", path, session_id);
            }
            Err(e) => {
                tracing::error!("Failed to watch {:?}: {}", path, e);
            }
        }
    }

    /// Handle an unwatch command
    async fn handle_unwatch(&mut self, node_id: NodeId) {
        if let Some(entry) = self.watches.remove(&node_id) {
            if let Err(e) = self.provider.unwatch(&entry.path).await {
                tracing::error!("Failed to unwatch {:?}: {}", entry.path, e);
            } else {
                tracing::debug!("Stopped watching {:?}", entry.path);
            }
            // handle is dropped here, which also cleans up provider resources
        }
    }

    /// Handle unwatch session command
    async fn handle_unwatch_session(&mut self, session_id: SessionId) {
        let mut to_remove = Vec::new();

        for (node_id, entry) in &mut self.watches {
            entry.sessions.retain(|s| *s != session_id);
            if entry.sessions.is_empty() {
                to_remove.push(*node_id);
            }
        }

        for node_id in to_remove {
            if let Some(entry) = self.watches.remove(&node_id) {
                if let Err(e) = self.provider.unwatch(&entry.path).await {
                    tracing::error!("Failed to unwatch {:?}: {}", entry.path, e);
                }
            }
        }
    }

    /// Handle unwatch all command
    async fn handle_unwatch_all(&mut self) {
        let entries: Vec<_> = self.watches.drain().collect();
        for (_, entry) in entries {
            if let Err(e) = self.provider.unwatch(&entry.path).await {
                tracing::error!("Failed to unwatch {:?}: {}", entry.path, e);
            }
        }
    }

    /// Route a raw [`FsChange`] from the provider to the matching sessions.
    fn dispatch_change(&self, change: FsChange) {
        for (node_id, entry) in &self.watches {
            if change.path.starts_with(&entry.path) {
                for session in &entry.sessions {
                    let evt = Event::FsChanged {
                        node: *node_id,
                        kind: change.kind.clone(),
                        session: *session,
                    };
                    send_or_warn(&self.events, evt, "emit FsChanged");
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
                        Ok(WatchCommand::Unwatch(node)) => {
                            self.handle_unwatch(node).await;
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
