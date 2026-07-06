use flume::Sender;
use std::path::PathBuf;

use rapidhash::fast::RandomState;
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
    Watch {
        location: LocationRef,
        session: SessionId,
        request: Option<RequestId>,
        event_mode: WatchEventMode,
    },
    Unwatch {
        location: LocationRef,
        scope: UnwatchScope,
    },
    UnwatchSession(SessionId),
    UnwatchAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchEventMode {
    Location,
    Compat { node: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnwatchScope {
    Session(SessionId),
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchSubscription {
    session: SessionId,
    event_mode: WatchEventMode,
}

/// Tracks which sessions are watching which paths
struct WatchEntry {
    path: PathBuf,
    location: LocationRef,
    refresh_node: Option<NodeId>,
    subscriptions: Vec<WatchSubscription>,
    #[allow(dead_code)]
    handle: Box<dyn WatchHandle>,
}

/// Watcher actor - monitors filesystem changes via a pluggable [`WatchProvider`].
pub struct Watcher {
    commands: flume::Receiver<WatchCommand>,
    events: Sender<Event>,
    registry: NodeRegistry,
    watches: scc::HashMap<LocationRef, WatchEntry, RandomState>,
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
            watches: scc::HashMap::with_hasher(RandomState::new()),
            provider,
            change_rx,
            change_tx,
            refresh_tx,
        }
    }

    async fn handle_watch(
        &mut self,
        location_ref: LocationRef,
        session_id: SessionId,
        request: Option<RequestId>,
        event_mode: WatchEventMode,
    ) {
        let location = match self.registry.resolve_location_ref(&location_ref) {
            Ok(location) => location,
            Err(error) => {
                self.emit_watch_error(error, session_id, request, "watch location resolve error");
                return;
            }
        };

        let path = match location.route().require_direct_path() {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                self.emit_watch_error(error, session_id, request, "watch location route error");
                return;
            }
        };

        let key = LocationRef::from_location(&location);
        let refresh_node = self.registry.register_location_node(location).ok();
        let subscription = WatchSubscription {
            session: session_id,
            event_mode,
        };

        if self
            .watches
            .update_sync(&key, |_, entry| {
                if !entry.subscriptions.contains(&subscription) {
                    entry.subscriptions.push(subscription.clone());
                }
            })
            .is_some()
        {
            return;
        }

        match self.provider.watch(&path, self.change_tx.clone()).await {
            Ok(handle) => {
                let _ = self.watches.insert_sync(
                    key.clone(),
                    WatchEntry {
                        path: path.clone(),
                        location: key,
                        refresh_node,
                        subscriptions: vec![subscription],
                        handle,
                    },
                );
            }
            Err(error) => {
                self.emit_watch_error(error, session_id, request, "watch location provider error");
            }
        }
    }

    fn emit_watch_error(
        &self,
        error: crate::CoreError,
        session_id: SessionId,
        request: Option<RequestId>,
        label: &'static str,
    ) {
        let event = match request {
            Some(request) => Event::from_request_error(error, session_id, request),
            None => Event::from_error(error, session_id),
        };
        send_or_warn(&self.events, event, label);
    }

    async fn handle_unwatch(&mut self, location_ref: LocationRef, scope: UnwatchScope) {
        let Ok(location) = self.registry.resolve_location_ref(&location_ref) else {
            tracing::warn!("Cannot unwatch unresolved location");
            return;
        };

        let key = LocationRef::from_location(&location);
        if let Some((_, mut entry)) = self.watches.remove_sync(&key) {
            match scope {
                UnwatchScope::Session(session_id) => {
                    entry
                        .subscriptions
                        .retain(|subscription| subscription.session != session_id);
                    if entry.subscriptions.is_empty() {
                        if let Err(e) = self.provider.unwatch(&entry.path).await {
                            tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch location");
                        }
                    } else {
                        let _ = self.watches.insert_sync(key, entry);
                    }
                }
                UnwatchScope::All => {
                    if let Err(e) = self.provider.unwatch(&entry.path).await {
                        tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch location");
                    }
                }
            }
        }
    }

    /// Handle unwatch session command
    async fn handle_unwatch_session(&mut self, session_id: SessionId) {
        let mut keys = Vec::new();
        self.watches.iter_sync(|key, entry| {
            if entry
                .subscriptions
                .iter()
                .any(|subscription| subscription.session == session_id)
            {
                keys.push(key.clone());
            }
            true
        });

        for key in keys {
            let Some((_, mut entry)) = self.watches.remove_sync(&key) else {
                continue;
            };
            entry
                .subscriptions
                .retain(|subscription| subscription.session != session_id);
            if entry.subscriptions.is_empty() {
                if let Err(e) = self.provider.unwatch(&entry.path).await {
                    tracing::error!(path = %entry.path.display(), error = %e, "Failed to unwatch path for session cleanup");
                }
            } else {
                let _ = self.watches.insert_sync(key, entry);
            }
        }
    }

    /// Handle unwatch all command
    async fn handle_unwatch_all(&mut self) {
        let mut keys = Vec::new();
        self.watches.iter_sync(|key, _| {
            keys.push(key.clone());
            true
        });
        for key in keys {
            let Some((_, entry)) = self.watches.remove_sync(&key) else {
                continue;
            };
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
        self.watches.iter_sync(|_key, entry| {
            if change.path.starts_with(&entry.path) {
                for subscription in &entry.subscriptions {
                    tracing::debug!(
                        path = %change.path.display(),
                        kind = ?change.kind,
                        session = ?subscription.session,
                        "Watcher dispatching filesystem change"
                    );
                    let evt = match subscription.event_mode {
                        WatchEventMode::Location => Event::FsChanged {
                            location: entry.location.clone(),
                            kind: change.kind.clone(),
                            session: subscription.session,
                        },
                        WatchEventMode::Compat { node } => Event::FsChangedCompat {
                            node,
                            kind: change.kind.clone(),
                            session: subscription.session,
                        },
                    };
                    send_or_warn(&self.events, evt, "emit filesystem change");
                }

                if let (Some(refresh_tx), Some(node)) = (&self.refresh_tx, entry.refresh_node) {
                    send_or_warn(
                        refresh_tx,
                        NavCommand::Invalidate(node),
                        "watch refresh invalidate",
                    );
                }
            }
            true
        });
    }
}

impl Actor for Watcher {
    async fn run(mut self) {
        tracing::debug!("Watcher actor started");

        loop {
            tokio::select! {
                cmd = self.commands.recv_async() => {
                    match cmd {
                        Ok(WatchCommand::Watch { location, session, request, event_mode }) => {
                            self.handle_watch(location, session, request, event_mode).await;
                        }
                        Ok(WatchCommand::Unwatch { location, scope }) => {
                            self.handle_unwatch(location, scope).await;
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
