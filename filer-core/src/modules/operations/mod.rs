//! Operations module — file system write operations.
//!
//! This module owns the Operator actor and registers handlers for:
//! - `ops.copy` — copy direct-local Locations
//! - `ops.copy.node.compat` — compatibility copy by NodeId
//! - `ops.move` — move direct-local Locations
//! - `ops.move.node.compat` — compatibility move by NodeId
//! - `ops.delete` — delete direct-local Locations
//! - `ops.delete.node.compat` — compatibility delete by NodeId
//! - `ops.rename` — rename a direct-local Location
//! - `ops.rename.node.compat` — compatibility rename by NodeId
//! - `ops.create_folder` — create a folder in a direct-local Location
//! - `ops.create_folder.node.compat` — compatibility create-folder by NodeId
//! - `ops.create_file` — create a file in a direct-local Location
//! - `ops.create_file.node.compat` — compatibility create-file by NodeId
//! - `ops.cancel` — cancel a specific active operation

pub mod operator;

use std::sync::Arc;

use crate::FsProvider;
use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::services::dir_cache::SharedDirCache;
use flume::Sender;
use operator::{Operator, OpsCommand};

/// Operations module — owns the Operator actor.
pub struct OperationsModule {
    provider: Arc<dyn FsProvider>,
    ops_tx: Sender<OpsCommand>,
    ops_rx: Option<flume::Receiver<OpsCommand>>,
    cache: Option<SharedDirCache>,
}

impl OperationsModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            ops_tx: tx,
            ops_rx: Some(rx),
            cache: None,
        }
    }

    pub fn with_cache(provider: Arc<dyn FsProvider>, cache: SharedDirCache) -> Self {
        let (tx, rx) = flume::unbounded();
        Self {
            provider,
            ops_tx: tx,
            ops_rx: Some(rx),
            cache: Some(cache),
        }
    }
}

impl Module for OperationsModule {
    fn init(mut self: Box<Self>, ctx: ModuleContext<'_>) {
        let ops_rx = self.ops_rx.take().expect("ScanModule already initialized");
        // ── Copy ─────────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.copy.node.compat", move |cmd, _ctx| {
            if let Command::CopyNodeCompat {
                sources,
                destination,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Copy {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.copy", move |cmd, _ctx| {
            if let Command::Copy {
                sources,
                destination,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::CopyLocation {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Move ─────────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.move.node.compat", move |cmd, _ctx| {
            if let Command::MoveNodeCompat {
                sources,
                destination,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Move {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.move", move |cmd, _ctx| {
            if let Command::Move {
                sources,
                destination,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::MoveLocation {
                    sources,
                    destination,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Delete ───────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.delete.node.compat", move |cmd, _ctx| {
            if let Command::DeleteNodeCompat {
                nodes,
                trash,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Delete {
                    targets: nodes,
                    trash,
                    session,
                    request,
                    operation,
                });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.delete", move |cmd, _ctx| {
            if let Command::Delete {
                locations,
                trash,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::DeleteLocation {
                    targets: locations,
                    trash,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Rename ───────────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.rename.node.compat", move |cmd, _ctx| {
            if let Command::RenameNodeCompat {
                node,
                new_name,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::Rename {
                    source: node,
                    new_name,
                    session,
                    request,
                    operation,
                });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.rename", move |cmd, _ctx| {
            if let Command::Rename {
                location,
                new_name,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::RenameLocation {
                    source: location,
                    new_name,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Create folder ────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers
            .on("ops.create_folder.node.compat", move |cmd, _ctx| {
                if let Command::CreateFolderNodeCompat {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                } = cmd
                {
                    let _ = tx.send(OpsCommand::CreateFolder {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    });
                }
            });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.create_folder", move |cmd, _ctx| {
            if let Command::CreateFolder {
                parent,
                name,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::CreateFolderLocation {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Create file ──────────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers
            .on("ops.create_file.node.compat", move |cmd, _ctx| {
                if let Command::CreateFileNodeCompat {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                } = cmd
                {
                    let _ = tx.send(OpsCommand::CreateFile {
                        parent,
                        name,
                        session,
                        request,
                        operation,
                    });
                }
            });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.create_file", move |cmd, _ctx| {
            if let Command::CreateFile {
                parent,
                name,
                session,
                request,
                operation,
            } = cmd
            {
                let _ = tx.send(OpsCommand::CreateFileLocation {
                    parent,
                    name,
                    session,
                    request,
                    operation,
                });
            }
        });

        // ── Cancel operation ─────────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.cancel", move |cmd, _ctx| {
            if let Command::CancelOperation { session, operation } = cmd {
                let _ = tx.send(OpsCommand::CancelOperation { session, operation });
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = self.ops_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(OpsCommand::Cancel(session));
        });

        // ── Spawn Operator actor ─────────────────────────────────────
        let operator = match self.cache.take() {
            Some(cache) => Operator::with_cache(
                ops_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
                cache,
            ),
            None => Operator::new(
                ops_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
            ),
        };
        ctx.actors.spawn(operator);
    }
}
