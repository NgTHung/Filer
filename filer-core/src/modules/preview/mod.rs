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
use crate::api::module::{Module, ModuleContext};
use crate::vfs::provider::FsProvider;
use previewer::{PreviewCommand, Previewer};

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
            .on("preview.load.node.compat", move |cmd, _ctx| {
                if let Command::LoadPreviewNodeCompat {
                    id,
                    options,
                    session,
                    request,
                } = cmd
                {
                    let _ = tx.send(PreviewCommand::Generate {
                        path: id,
                        options,
                        session,
                        request,
                    });
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
                let _ = tx.send(PreviewCommand::GenerateLocation {
                    location,
                    options,
                    session,
                    request,
                });
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers.on("preview.cancel", move |cmd, _ctx| {
            if let Command::CancelPreview { session } = cmd {
                let _ = tx.send(PreviewCommand::Cancel(session));
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers
            .on("metadata.load.node.compat", move |cmd, _ctx| {
                if let Command::LoadMetadataNodeCompat {
                    node,
                    session,
                    request,
                } = cmd
                {
                    let _ = tx.send(PreviewCommand::LoadMetadata(node, session, request));
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
                let _ = tx.send(PreviewCommand::LoadMetadataLocation(
                    location, session, request,
                ));
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers
            .on("metadata.extended.node.compat", move |cmd, _ctx| {
                if let Command::LoadExtendedMetadataNodeCompat {
                    node,
                    session,
                    request,
                } = cmd
                {
                    let _ = tx.send(PreviewCommand::LoadExtendedMetadata(node, session, request));
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
                let _ = tx.send(PreviewCommand::LoadExtendedMetadataLocation(
                    location, session, request,
                ));
            }
        });

        let tx = preview_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(PreviewCommand::Cancel(session));
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
