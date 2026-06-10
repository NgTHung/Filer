//! Watch module — filesystem change monitoring.
//!
//! This module owns the Watcher actor and registers handlers for:
//! - `watch` — watch a direct-local Location for changes
//! - `watch.node.compat` — compatibility watch by NodeId
//! - `watch.remove` — stop watching a direct-local Location
//! - `watch.node.remove.compat` — compatibility unwatch by NodeId
//! - `watch.session_remove` — stop all watches for a session
//!
//! `WatchModule::new` emits filesystem change events only. Use
//! `WatchModule::with_refresh` to also route watched-root changes through
//! navigation invalidation so current directory views refresh after cache
//! invalidation.

pub mod watcher;

use std::sync::Arc;

use flume::Sender;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::modules::navigation::navigator::NavCommand;
use crate::vfs::watch::WatchProvider;
use watcher::WatchCommand;

/// Watch module — owns the Watcher actor.
///
/// Accepts any [`WatchProvider`] so that the underlying watch mechanism
/// can vary per filesystem backend (notify for local, polling for remote, etc.).
pub struct WatchModule {
    provider: Arc<dyn WatchProvider>,
    refresh_tx: Option<Sender<NavCommand>>,
}

impl WatchModule {
    pub fn new(provider: Arc<dyn WatchProvider>) -> Self {
        Self {
            provider,
            refresh_tx: None,
        }
    }

    pub fn with_refresh(provider: Arc<dyn WatchProvider>, refresh_tx: Sender<NavCommand>) -> Self {
        Self {
            provider,
            refresh_tx: Some(refresh_tx),
        }
    }
}

impl Module for WatchModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (watch_tx, watch_rx) = flume::unbounded::<WatchCommand>();

        let tx = watch_tx.clone();
        ctx.handlers.on("watch.node.compat", move |cmd, _ctx| {
            if let Command::WatchNodeCompat { node, session } = cmd {
                let _ = tx.send(WatchCommand::Watch(node, session));
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on("watch", move |cmd, _ctx| {
            if let Command::Watch {
                location,
                session,
                request,
            } = cmd
            {
                let _ = tx.send(WatchCommand::WatchLocation {
                    location,
                    session,
                    request,
                });
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers
            .on("watch.node.remove.compat", move |cmd, _ctx| {
                if let Command::UnwatchNodeCompat { node } = cmd {
                    let _ = tx.send(WatchCommand::Unwatch(node));
                }
            });

        let tx = watch_tx.clone();
        ctx.handlers.on("watch.remove", move |cmd, _ctx| {
            if let Command::Unwatch { location, session } = cmd {
                let _ = tx.send(WatchCommand::UnwatchLocation { location, session });
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on("watch.session_remove", move |cmd, _ctx| {
            if let Command::UnwatchSession(session) = cmd {
                let _ = tx.send(WatchCommand::UnwatchSession(session));
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(WatchCommand::UnwatchSession(session));
        });

        let watcher = match self.refresh_tx {
            Some(refresh_tx) => watcher::Watcher::with_refresh(
                watch_rx,
                ctx.events.clone(),
                ctx.registry.clone(),
                self.provider,
                refresh_tx,
            ),
            None => watcher::Watcher::new(
                watch_rx,
                ctx.events.clone(),
                ctx.registry.clone(),
                self.provider,
            ),
        };
        ctx.actors.spawn(watcher);
    }
}
