//! Preview module — file preview generation and metadata loading.
//!
//! This module owns the Previewer actor and registers handlers for:
//! - `preview.load` — generate preview for a Location
//! - `preview.load.node.compat` — compatibility preview by NodeId
//! - `preview.cancel` — cancel ongoing preview
//! - `metadata.load` — load basic metadata for a Location
//! - `metadata.load.node.compat` — compatibility metadata by NodeId
//! - `metadata.extended` — load extended metadata for a Location
//! - `metadata.extended.node.compat` — compatibility extended metadata by NodeId

pub mod previewer;

use std::sync::Arc;

use crate::api::commands::Command;
use crate::api::events::Event;
use crate::api::module::{Module, ModuleContext};
use crate::errors::CoreError;
use crate::utils::channel::send_or_warn;
use crate::vfs::provider::FsProvider;
use previewer::{PreviewCommand, PreviewEventMode, Previewer};

/// Preview module — owns the Previewer actor.
pub struct PreviewModule {
    provider: Arc<dyn FsProvider>,
}

impl PreviewModule {
    pub fn new(provider: Arc<dyn FsProvider>) -> Self {
        Self { provider }
    }
}

impl Module for PreviewModule {
    fn init(self: Box<Self>, ctx: ModuleContext<'_>) {
        let (preview_tx, preview_rx) = flume::unbounded();

        let tx = preview_tx.clone();
        ctx.handlers
            .on("preview.load.node.compat", move |cmd, ctx| {
                if let Command::LoadPreviewNodeCompat {
                    id,
                    options,
                    session,
                    request,
                } = cmd
                {
                    let Some(location) = ctx.registry.resolve_node_location(id) else {
                        send_or_warn(
                            &ctx.events,
                            Event::from_request_error(
                                CoreError::invalid_input(format!("Unable to resolve ID: {id:?}")),
                                session,
                                request,
                            ),
                            "preview.load.node.compat resolve",
                        );
                        return;
                    };
                    send_or_warn(
                        &tx,
                        PreviewCommand::Generate {
                            location,
                            options,
                            event_mode: PreviewEventMode::Compat { node: id },
                            session,
                            request,
                        },
                        "preview.load.node.compat",
                    );
                }
            });

        let tx = preview_tx.clone();
        ctx.handlers.on("preview.load", move |cmd, _ctx| {
            if let Command::LoadPreview {
                location,
                options,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    PreviewCommand::Generate {
                        location,
                        options,
                        event_mode: PreviewEventMode::Location,
                        session,
                        request,
                    },
                    "preview.load",
                );
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers.on("preview.cancel", move |cmd, _ctx| {
            if let Command::CancelPreview { session } = cmd {
                send_or_warn(&tx, PreviewCommand::Cancel(session), "preview.cancel");
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers
            .on("metadata.load.node.compat", move |cmd, ctx| {
                if let Command::LoadMetadataNodeCompat {
                    node,
                    session,
                    request,
                } = cmd
                {
                    let Some(location) = ctx.registry.resolve_node_location(node) else {
                        send_or_warn(
                            &ctx.events,
                            Event::from_request_error(
                                CoreError::invalid_input(format!("Unable to resolve ID: {node:?}")),
                                session,
                                request,
                            ),
                            "metadata.load.node.compat resolve",
                        );
                        return;
                    };
                    send_or_warn(
                        &tx,
                        PreviewCommand::LoadMetadata {
                            location,
                            event_mode: PreviewEventMode::Compat { node },
                            session,
                            request,
                        },
                        "metadata.load.node.compat",
                    );
                }
            });

        let tx = preview_tx.clone();
        ctx.handlers.on("metadata.load", move |cmd, _ctx| {
            if let Command::LoadMetadata {
                location,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    PreviewCommand::LoadMetadata {
                        location,
                        event_mode: PreviewEventMode::Location,
                        session,
                        request,
                    },
                    "metadata.load",
                );
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers
            .on("metadata.extended.node.compat", move |cmd, ctx| {
                if let Command::LoadExtendedMetadataNodeCompat {
                    node,
                    session,
                    request,
                } = cmd
                {
                    let Some(location) = ctx.registry.resolve_node_location(node) else {
                        send_or_warn(
                            &ctx.events,
                            Event::from_request_error(
                                CoreError::invalid_input(format!("Unable to resolve ID: {node:?}")),
                                session,
                                request,
                            ),
                            "metadata.extended.node.compat resolve",
                        );
                        return;
                    };
                    send_or_warn(
                        &tx,
                        PreviewCommand::LoadExtendedMetadata {
                            location,
                            event_mode: PreviewEventMode::Compat { node },
                            session,
                            request,
                        },
                        "metadata.extended.node.compat",
                    );
                }
            });

        let tx = preview_tx.clone();
        ctx.handlers.on("metadata.extended", move |cmd, _ctx| {
            if let Command::LoadExtendedMetadata {
                location,
                session,
                request,
            } = cmd
            {
                send_or_warn(
                    &tx,
                    PreviewCommand::LoadExtendedMetadata {
                        location,
                        event_mode: PreviewEventMode::Location,
                        session,
                        request,
                    },
                    "metadata.extended",
                );
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            send_or_warn(
                &tx,
                PreviewCommand::Cancel(session),
                "preview session destroy",
            );
        });

        let previewer = Previewer::new(
            preview_rx,
            ctx.events.clone(),
            self.provider.clone(),
            ctx.registry.clone(),
        );
        ctx.actors.spawn(previewer);
    }
}
