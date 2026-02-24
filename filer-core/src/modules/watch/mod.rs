//! Watch module — filesystem change monitoring.
//!
//! This module owns the Watcher actor and registers handlers for:
//! - `watch` — watch a directory for changes
//! - `watch.remove` — stop watching a directory
//! - `watch.session_remove` — stop all watches for a session

pub mod watcher;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use watcher::WatchCommand;

/// Watch module — owns the Watcher actor.
pub struct WatchModule;

impl WatchModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for WatchModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (watch_tx, _watch_rx) = flume::unbounded::<WatchCommand>();

        // ── Watch ────────────────────────────────────────────────────
        let tx = watch_tx.clone();
        ctx.handlers.on("watch", move |cmd, _ctx| {
            if let Command::Watch(node, session) = cmd {
                let _ = tx.send(WatchCommand::Watch(node, session));
            }
        });

        // ── Unwatch ──────────────────────────────────────────────────
        let tx = watch_tx.clone();
        ctx.handlers.on("watch.remove", move |cmd, _ctx| {
            if let Command::Unwatch(node) = cmd {
                let _ = tx.send(WatchCommand::Unwatch(node));
            }
        });

        // ── Unwatch session ──────────────────────────────────────────
        let tx = watch_tx.clone();
        ctx.handlers.on("watch.session_remove", move |cmd, _ctx| {
            if let Command::UnwatchSession(session) = cmd {
                let _ = tx.send(WatchCommand::UnwatchSession(session));
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = watch_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(WatchCommand::UnwatchSession(session));
        });

        // TODO: Spawn Watcher actor once constructor is implemented
        // let watcher = Watcher::new(watch_rx, ctx.events.clone());
        // ctx.actors.spawn(watcher);
    }
}
