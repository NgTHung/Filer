//! Preview module — file preview generation and metadata loading.
//!
//! This module owns the Previewer actor and registers handlers for:
//! - `preview.load` — generate preview for a file
//! - `preview.cancel` — cancel ongoing preview
//! - `metadata.load` — load basic metadata
//! - `metadata.extended` — load extended metadata (EXIF, ID3, etc.)

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

        // ── Load preview ─────────────────────────────────────────────
        let tx = preview_tx.clone();
        ctx.handlers.on("preview.load", move |cmd, _ctx| {
            if let Command::LoadPreview { id, options, session } = cmd {
                let _ = tx.send(PreviewCommand::Generate { path: id, options, session });
            }
        });

        // ── Cancel preview ───────────────────────────────────────────
        let tx = preview_tx.clone();
        ctx.handlers.on("preview.cancel", move |cmd, _ctx| {
            if let Command::CancelPreview(session) = cmd {
                let _ = tx.send(PreviewCommand::Cancel(session));
            }
        });

        // ── Load metadata ────────────────────────────────────────────
        let tx = preview_tx.clone();
        ctx.handlers.on("metadata.load", move |cmd, _ctx| {
            if let Command::LoadMetadata(node, session) = cmd {
                let _ = tx.send(PreviewCommand::LoadMetadata(node, session));
            }
        });

        // ── Load extended metadata ───────────────────────────────────
        let tx = preview_tx.clone();
        ctx.handlers.on("metadata.extended", move |cmd, _ctx| {
            if let Command::LoadExtendedMetadata(node, session) = cmd {
                let _ = tx.send(PreviewCommand::LoadExtendedMetadata(node, session));
            }
        });

        // ── Session cleanup hook ─────────────────────────────────────
        let tx = preview_tx.clone();
        ctx.handlers.on_session_destroy(move |session, _ctx| {
            let _ = tx.send(PreviewCommand::Cancel(session));
        });

        // ── Spawn Previewer actor ────────────────────────────────────
        let previewer = Previewer::new(
            preview_rx,
            ctx.events.clone(),
            self.provider.clone(),
            ctx.registry.clone(),
        );
        ctx.actors.spawn(previewer);
    }
}
