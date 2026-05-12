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

use super::scan::scanner::ScanCommand;
use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::utils::channel::send_or_warn;
use navigator::{NavCommand, Navigator};

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
            if let Command::Navigate {
                path,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    NavCommand::NavigateToPath {
                        session,
                        path,
                        request,
                    },
                    "navigate",
                );
            }
        });

        // ── Navigate to node ─────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.node", move |cmd, _ctx| {
            if let Command::NavigateToNode {
                node,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    NavCommand::Navigate {
                        session,
                        node,
                        request,
                    },
                    "navigate.node",
                );
            }
        });

        // ── Navigate up ──────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.up", move |cmd, _ctx| {
            if let Command::NavigateUp { session, request } = cmd {
                send_or_warn(&tx, NavCommand::Up(session, request), "navigate.up");
            }
        });

        // ── Navigate back ────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.back", move |cmd, _ctx| {
            if let Command::NavigateBack { session, request } = cmd {
                send_or_warn(&tx, NavCommand::Back(session, request), "navigate.back");
            }
        });

        // ── Navigate forward ─────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.forward", move |cmd, _ctx| {
            if let Command::NavigateForward { session, request } = cmd {
                send_or_warn(
                    &tx,
                    NavCommand::Forward(session, request),
                    "navigate.forward",
                );
            }
        });

        // ── Refresh ──────────────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.refresh", move |cmd, _ctx| {
            if let Command::Refresh { session, request } = cmd {
                send_or_warn(
                    &tx,
                    NavCommand::Refresh(session, request),
                    "navigate.refresh",
                );
            }
        });

        // ── Pipeline state ──────────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on("navigate.pipeline", move |cmd, _ctx| {
            if let Command::SetPipeline { session, config } = cmd {
                send_or_warn(
                    &tx,
                    NavCommand::SetPipeline { session, config },
                    "navigate.pipeline",
                );
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = nav_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            send_or_warn(
                &tx,
                NavCommand::RemoveSession(session),
                "nav session cleanup",
            );
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
