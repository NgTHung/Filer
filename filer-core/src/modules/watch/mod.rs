//! Watch module — filesystem change monitoring.
//!
//! This module owns the Watcher actor and registers handlers for:
//! - `watch` — watch a direct-local Location for changes
//! - `watch.remove` — stop watching a direct-local Location
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
use crate::utils::channel::send_or_warn;
use crate::vfs::watch::WatchProvider;
use watcher::{UnwatchScope, WatchCommand, WatchEventMode};

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
        ctx.handlers.on("watch", move |cmd, _ctx| {
            if let Command::Watch {
                location,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    WatchCommand::Watch {
                        location,
                        session,
                        request: Some(request),
                        event_mode: WatchEventMode::Location,
                    },
                    "watch",
                );
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on("watch.remove", move |cmd, _ctx| {
            if let Command::Unwatch { location, session } = cmd {
                send_or_warn(
                    &tx,
                    WatchCommand::Unwatch {
                        location,
                        scope: UnwatchScope::Session(session),
                    },
                    "watch.remove",
                );
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on("watch.session_remove", move |cmd, _ctx| {
            if let Command::UnwatchSession(session) = cmd {
                send_or_warn(
                    &tx,
                    WatchCommand::UnwatchSession(session),
                    "watch.session_remove",
                );
            }
        });

        let tx = watch_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            send_or_warn(
                &tx,
                WatchCommand::UnwatchSession(session),
                "watch session destroy",
            );
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
