//! Operations module — file system write operations.
//!
//! This module owns the Operator actor and registers handlers for:
//! - `ops.copy` — copy direct-local Locations
//! - `ops.move` — move direct-local Locations
//! - `ops.delete` — delete direct-local Locations
//! - `ops.rename` — rename a direct-local Location
//! - `ops.create_folder` — create a folder in a direct-local Location
//! - `ops.create_file` — create a file in a direct-local Location
//! - `ops.cancel` — cancel a specific active operation

pub mod operator;
pub mod target;

use std::sync::Arc;

use crate::FsProvider;
use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn;
use flume::Sender;
use operator::{OperationEventMode, Operator, OpsCommand};

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
                send_or_warn(
                    &tx,
                    OpsCommand::Copy {
                        sources,
                        destination,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.copy",
                );
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
                send_or_warn(
                    &tx,
                    OpsCommand::Move {
                        sources,
                        destination,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.move",
                );
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
                send_or_warn(
                    &tx,
                    OpsCommand::Delete {
                        targets: locations,
                        trash,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.delete",
                );
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
                send_or_warn(
                    &tx,
                    OpsCommand::Rename {
                        source: location,
                        new_name,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.rename",
                );
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
                send_or_warn(
                    &tx,
                    OpsCommand::CreateFolder {
                        parent,
                        name,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.create_folder",
                );
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
                send_or_warn(
                    &tx,
                    OpsCommand::CreateFile {
                        parent,
                        name,
                        event_mode: OperationEventMode::Location,
                        session,
                        request,
                        operation,
                    },
                    "ops.create_file",
                );
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.cancel", move |cmd, _ctx| {
            if let Command::CancelOperation { session, operation } = cmd {
                send_or_warn(
                    &tx,
                    OpsCommand::CancelOperation { session, operation },
                    "ops.cancel",
                );
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            send_or_warn(
                &tx,
                OpsCommand::Cancel(session),
                "operations session destroy",
            );
        });

        let operator = match self.cache.take() {
            Some(cache) => Operator::with_cache(
                ops_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
                cache,
            )
            .with_work_tracker(ctx.actors.work_tracker()),
            None => Operator::new(
                ops_rx,
                ctx.events.clone(),
                self.provider.clone(),
                ctx.registry.clone(),
            )
            .with_work_tracker(ctx.actors.work_tracker()),
        };
        ctx.actors.spawn(operator);
    }
}
