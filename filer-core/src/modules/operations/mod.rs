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
pub mod target;

use std::sync::Arc;

use crate::CoreError;
use crate::FsProvider;
use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{Module, ModuleContext};
use crate::model::location::LocationRef;
use crate::model::node::NodeId;
use crate::model::operation::OperationId;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn;
use flume::Sender;
use operator::{OperationEventMode, Operator, OpsCommand};

fn resolve_compat_node(
    registry: &crate::model::registry::NodeRegistry,
    node: NodeId,
) -> Result<LocationRef, CoreError> {
    registry
        .resolve_node_location(node)
        .ok_or_else(|| CoreError::invalid_input(format!("Cannot resolve node {node:?}")))
}

fn emit_compat_resolve_error(
    events: &Sender<Event>,
    error: CoreError,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
    context: &'static str,
) {
    send_or_warn(
        events,
        Event::from_operation_error(error, session, request, operation),
        context,
    );
}

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
        ctx.handlers.on("ops.copy.node.compat", move |cmd, _ctx| {
            if let Command::CopyNodeCompat {
                sources,
                destination,
                session,
                request,
                operation,
            } = cmd
            {
                let resolved = sources
                    .into_iter()
                    .map(|source| resolve_compat_node(&_ctx.registry, source))
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|sources| {
                        resolve_compat_node(&_ctx.registry, destination)
                            .map(|destination| (sources, destination))
                    });
                match resolved {
                    Ok((sources, destination)) => {
                        let _ = tx.send(OpsCommand::Copy {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                    Err(error) => emit_compat_resolve_error(
                        &_ctx.events,
                        error,
                        session,
                        request,
                        operation,
                        "operations: copy compat resolve",
                    ),
                }
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
                let _ = tx.send(OpsCommand::Copy {
                    sources,
                    destination,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

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
                let resolved = sources
                    .into_iter()
                    .map(|source| resolve_compat_node(&_ctx.registry, source))
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|sources| {
                        resolve_compat_node(&_ctx.registry, destination)
                            .map(|destination| (sources, destination))
                    });
                match resolved {
                    Ok((sources, destination)) => {
                        let _ = tx.send(OpsCommand::Move {
                            sources,
                            destination,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                    Err(error) => emit_compat_resolve_error(
                        &_ctx.events,
                        error,
                        session,
                        request,
                        operation,
                        "operations: move compat resolve",
                    ),
                }
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
                let _ = tx.send(OpsCommand::Move {
                    sources,
                    destination,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

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
                let resolved = nodes
                    .into_iter()
                    .map(|node| resolve_compat_node(&_ctx.registry, node))
                    .collect::<Result<Vec<_>, _>>();
                match resolved {
                    Ok(targets) => {
                        let _ = tx.send(OpsCommand::Delete {
                            targets,
                            trash,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                    Err(error) => emit_compat_resolve_error(
                        &_ctx.events,
                        error,
                        session,
                        request,
                        operation,
                        "operations: delete compat resolve",
                    ),
                }
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
                let _ = tx.send(OpsCommand::Delete {
                    targets: locations,
                    trash,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

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
                match resolve_compat_node(&_ctx.registry, node) {
                    Ok(source) => {
                        let _ = tx.send(OpsCommand::Rename {
                            source,
                            new_name,
                            event_mode: OperationEventMode::Compat,
                            session,
                            request,
                            operation,
                        });
                    }
                    Err(error) => emit_compat_resolve_error(
                        &_ctx.events,
                        error,
                        session,
                        request,
                        operation,
                        "operations: rename compat resolve",
                    ),
                }
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
                let _ = tx.send(OpsCommand::Rename {
                    source: location,
                    new_name,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

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
                    match resolve_compat_node(&_ctx.registry, parent) {
                        Ok(parent) => {
                            let _ = tx.send(OpsCommand::CreateFolder {
                                parent,
                                name,
                                event_mode: OperationEventMode::Compat,
                                session,
                                request,
                                operation,
                            });
                        }
                        Err(error) => emit_compat_resolve_error(
                            &_ctx.events,
                            error,
                            session,
                            request,
                            operation,
                            "operations: create folder compat resolve",
                        ),
                    }
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
                let _ = tx.send(OpsCommand::CreateFolder {
                    parent,
                    name,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

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
                    match resolve_compat_node(&_ctx.registry, parent) {
                        Ok(parent) => {
                            let _ = tx.send(OpsCommand::CreateFile {
                                parent,
                                name,
                                event_mode: OperationEventMode::Compat,
                                session,
                                request,
                                operation,
                            });
                        }
                        Err(error) => emit_compat_resolve_error(
                            &_ctx.events,
                            error,
                            session,
                            request,
                            operation,
                            "operations: create file compat resolve",
                        ),
                    }
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
                let _ = tx.send(OpsCommand::CreateFile {
                    parent,
                    name,
                    event_mode: OperationEventMode::Location,
                    session,
                    request,
                    operation,
                });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on("ops.cancel", move |cmd, _ctx| {
            if let Command::CancelOperation { session, operation } = cmd {
                let _ = tx.send(OpsCommand::CancelOperation { session, operation });
            }
        });

        let tx = self.ops_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(OpsCommand::Cancel(session));
        });

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
