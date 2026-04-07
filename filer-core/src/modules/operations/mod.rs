//! Operations module — file system write operations.
//!
//! This module owns the Operator actor and registers handlers for:
//! - `ops.copy` — copy files/directories
//! - `ops.move` — move files/directories
//! - `ops.delete` — delete (to trash or permanent)
//! - `ops.rename` — rename a file or directory
//! - `ops.create_folder` — create a new folder
//! - `ops.create_file` — create a new file

pub mod operator;

use std::sync::Arc;

use crate::FsProvider;
use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use flume::Sender;
use operator::{Operator, OpsCommand};

/// Operations module — owns the Operator actor.
pub struct OperationsModule {
    provider: Arc<dyn FsProvider>,
    ops_tx: Sender<OpsCommand>,
    ops_rx: Option<flume::Receiver<OpsCommand>>,
}

impl OperationsModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            ops_tx: tx,
            ops_rx: Some(rx),
        }
    }
}

impl Module for OperationsModule {
    fn init(mut self: Box<Self>, ctx: ModuleContext<'_>) {
        let ops_rx = self.ops_rx.take().expect("ScanModule already initialized");
        // ── Copy ─────────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.copy", move |cmd, _ctx| {
            if let Command::Copy {
                sources,
                destination,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Copy {
                    sources,
                    destination,
                    session,
                });
            }
        });

        // ── Move ─────────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.move", move |cmd, _ctx| {
            if let Command::Move {
                sources,
                destination,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Move {
                    sources,
                    destination,
                    session,
                });
            }
        });

        // ── Delete ───────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.delete", move |cmd, _ctx| {
            if let Command::Delete {
                nodes,
                trash,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Delete {
                    targets: nodes,
                    trash,
                    session,
                });
            }
        });

        // ── Rename ───────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.rename", move |cmd, _ctx| {
            if let Command::Rename {
                node,
                new_name,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Rename {
                    source: node,
                    new_name,
                    session,
                });
            }
        });

        // ── Create folder ────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.create_folder", move |cmd, _ctx| {
            if let Command::CreateFolder {
                parent,
                name,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::CreateFolder {
                    parent,
                    name,
                    session,
                });
            }
        });

        // ── Create file ──────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.create_file", move |cmd, _ctx| {
            if let Command::CreateFile {
                parent,
                name,
                session,
            } = cmd
            {
                let _ = tx.send(OpsCommand::CreateFile {
                    parent,
                    name,
                    session,
                });
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(OpsCommand::Cancel(session));
        });

        // ── Spawn Operator actor ─────────────────────────────────────
        let operator = Operator::new(ops_rx, ctx.events.clone(), self.provider.clone(), ctx.registry.clone());
        ctx.actors.spawn(operator);
    }
}
