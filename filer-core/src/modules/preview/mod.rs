//! Preview module — file preview generation and metadata loading.
//!
//! This module owns the Previewer actor and registers handlers for:
//! - `preview.load` — generate preview for a Location
//! - `preview.cancel` — cancel ongoing preview
//! - `metadata.load` — load basic metadata for a Location
//! - `metadata.extended` — load extended metadata for a Location

pub mod previewer;

use std::sync::Arc;

use crate::api::commands::Command;
use crate::api::module::{Module, ModuleContext};
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
        )
        .with_work_tracker(ctx.actors.work_tracker());
        ctx.actors.spawn(previewer);
    }
}
