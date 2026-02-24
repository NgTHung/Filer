//! Navigation module — directory browsing, history, and state management.
//!
//! This module owns the Navigator actor and registers handlers for:
//! - `navigate` — navigate to path
//! - `navigate.node` — navigate to NodeId
//! - `navigate.up` — go to parent
//! - `navigate.back` — go back in history
//! - `navigate.refresh` — refresh current directory
//!
//! # Dependencies
//!
//! The Navigator triggers scans via a `Sender<ScanCommand>`. You must
//! create a `ScanModule` first and pass its `sender()`:
//!
//! ```ignore
//! let scan = ScanModule::new(provider);
//! let nav = NavigationModule::new(scan.sender());
//! core.load(scan);
//! core.load(nav);
//! ```
//!
//! This separation lets you swap the scanner without touching navigation:
//!
//! ```ignore
//! let scan = ScanModule::new(Arc::new(MyFasterScanner::new()));
//! let nav = NavigationModule::new(scan.sender());
//! ```

pub mod navigator;

use flume::Sender;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use navigator::{NavCommand, Navigator};
use super::scan::scanner::ScanCommand;

/// Navigation module — owns the Navigator actor.
///
/// Takes a `Sender<ScanCommand>` so the Navigator can trigger directory
/// scans. Get this from `ScanModule::sender()`.
pub struct NavigationModule {
    scanner_tx: Sender<ScanCommand>,
}

impl NavigationModule {
    pub fn new(scanner_tx: Sender<ScanCommand>) -> Self {
        Self { scanner_tx }
    }
}

impl Module for NavigationModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (nav_tx, nav_rx) = flume::unbounded();

        // ── Navigate to path ─────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate", move |cmd, _ctx| {
            if let Command::Navigate(path, session) = cmd {
                let _ = tx.send(NavCommand::NavigateToPath { session, path });
            }
        });

        // ── Navigate to node ─────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.node", move |cmd, _ctx| {
            if let Command::NavigateToNode(node, session) = cmd {
                let _ = tx.send(NavCommand::Navigate { session, node });
            }
        });

        // ── Navigate up ──────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.up", move |cmd, _ctx| {
            if let Command::NavigateUp(session) = cmd {
                let _ = tx.send(NavCommand::Up(session));
            }
        });

        // ── Navigate back ────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.back", move |cmd, _ctx| {
            if let Command::NavigateBack(session) = cmd {
                let _ = tx.send(NavCommand::Back(session));
            }
        });

        // ── Refresh ──────────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.refresh", move |cmd, _ctx| {
            if let Command::Refresh(session) = cmd {
                let _ = tx.send(NavCommand::Refresh(session));
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(NavCommand::RemoveSession(session));
        });

        // ── Spawn Navigator actor ────────────────────────────────────
        let navigator = Navigator::new(
            nav_rx,
            ctx.events.clone(),
            self.scanner_tx.clone(),
            ctx.registry.clone(),
        );
        ctx.actors.spawn(navigator);
    }
}
