use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::node::{FileNode, NodeId};
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::services::mime::MimeDetector;
use crate::services::preview::PreviewCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::provider::FsProvider;
use crate::{MetadataRegistry, PreviewOptions, PreviewRegistry};

/// Commands for previewer actor
#[derive(Debug, Clone)]
pub enum PreviewCommand {
    /// Generate preview for a file
    Generate {
        path: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
    },
    /// Load basic metadata (NodeMeta) for a file
    LoadMetadata(NodeId, SessionId),
    /// Load extended metadata (EXIF, ID3, page count…) for a file
    LoadExtendedMetadata(NodeId, SessionId),
    /// Cancel all ongoing work for a session
    Cancel(SessionId),
    /// Drop all cached previews
    ClearCache,
}

#[derive(Clone)]
struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    fn new() -> Self {
        Self { cancelled: Arc::new(AtomicBool::new(false)) }
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Previewer actor — generates file previews and extracts metadata.
///
/// Each session can have at most one in-flight operation (preview generation
/// or extended-metadata extraction). Dispatching a new operation for a session
/// cancels the previous one.
pub struct Previewer {
    commands:          Receiver<PreviewCommand>,
    events:            Sender<Event>,
    preview_registry:  Arc<PreviewRegistry>,
    metadata_registry: Arc<MetadataRegistry>,
    cache:             Arc<Mutex<PreviewCache>>,
    provider:          Arc<dyn FsProvider>,
    registry:          NodeRegistry,
    active:            Arc<scc::HashMap<SessionId, CancellationToken, RandomState>>,
}

impl Previewer {
    pub fn new(
        commands: Receiver<PreviewCommand>,
        events:   Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            preview_registry:  Arc::new(PreviewRegistry::with_defaults()),
            metadata_registry: Arc::new(MetadataRegistry::with_defaults()),
            cache:             Arc::new(Mutex::new(
                PreviewCache::new(64 * 1024 * 1024, Duration::from_secs(300)),
            )),
            provider,
            registry,
            active: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    // ── Preview generation ────────────────────────────────────────────────────

    fn dispatch_preview(
        &self,
        node:    NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
    ) {
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(&self.events, Event::Error {
                message:     format!("Cannot resolve node {node:?}"),
                recoverable: true,
                session,
            }, "previewer: resolve");
            return;
        };

        // Check cache first — no spawn needed on hit.
        if let Ok(cache) = self.cache.lock() {
            if let Some(preview) = cache.get(&path) {
                send_or_warn(&self.events, Event::PreviewReady { node, preview, session }, "previewer: cache hit");
                return;
            }
        }

        let cancel = self.arm_cancel(session);
        let events           = self.events.clone();
        let preview_registry = self.preview_registry.clone();
        let cache            = self.cache.clone();
        let active           = self.active.clone();
        let opts             = options.unwrap_or_default();

        tokio::spawn(async move {
            if cancel.is_cancelled() { return; }

            let result = preview_registry.generate_with_options(&path, &opts).await;

            if cancel.is_cancelled() { return; }

            match result {
                Ok(preview) => {
                    // Store in cache before emitting.
                    if let Ok(mut c) = cache.lock() {
                        c.put(path, preview.clone());
                    }
                    send_or_warn_async(&events, Event::PreviewReady { node, preview, session }, "preview ready").await;
                }
                Err(e) => {
                    send_or_warn_async(&events, Event::PreviewFailed {
                        node,
                        reason: e.to_string(),
                        session,
                    }, "preview failed").await;
                }
            }

            let _ = active.remove_async(&session).await;
        });
    }

    // ── Basic metadata ────────────────────────────────────────────────────────

    fn dispatch_metadata(&self, node: NodeId, session: SessionId) {
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(&self.events, Event::Error {
                message:     format!("Cannot resolve node {node:?}"),
                recoverable: true,
                session,
            }, "previewer: resolve");
            return;
        };

        let events   = self.events.clone();
        let registry = self.registry.clone();

        tokio::spawn(async move {
            match FileNode::from_path(path, Some(registry)) {
                Ok(file_node) => {
                    send_or_warn_async(&events, Event::MetadataLoaded {
                        node,
                        meta: file_node.meta,
                        session,
                    }, "metadata loaded").await;
                }
                Err(e) => {
                    send_or_warn_async(&events, Event::Error {
                        message:     e.to_string(),
                        recoverable: true,
                        session,
                    }, "metadata error").await;
                }
            }
        });
    }

    // ── Extended metadata ─────────────────────────────────────────────────────

    fn dispatch_extended_metadata(&self, node: NodeId, session: SessionId) {
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(&self.events, Event::Error {
                message:     format!("Cannot resolve node {node:?}"),
                recoverable: true,
                session,
            }, "previewer: resolve");
            return;
        };

        let cancel            = self.arm_cancel(session);
        let events            = self.events.clone();
        let metadata_registry = self.metadata_registry.clone();
        let provider          = self.provider.clone();
        let active            = self.active.clone();

        tokio::spawn(async move {
            if cancel.is_cancelled() { return; }

            // Tier 1: extension-based MIME detection (zero I/O).
            // Upgrade to magic bytes if extension is ambiguous.
            let mime = {
                let ext_info = MimeDetector::detect_from_path(&path);
                if ext_info.confidence == crate::services::mime::DetectionConfidence::Definitive {
                    ext_info
                } else {
                    // Read 512-byte header through the provider.
                    let header = provider.read_header(&path, 512).await.ok();
                    MimeDetector::detect_with_strategy(
                        &path,
                        header.as_deref(),
                        crate::services::mime::DetectionStrategy::ExtensionWithFallback,
                    )
                }
            };

            if cancel.is_cancelled() { return; }

            match metadata_registry.extract(&path, &mime, provider.as_ref()).await {
                Ok(extended) => {
                    send_or_warn_async(&events, Event::ExtendedMetadataLoaded { node, extended, session }, "extended metadata").await;
                }
                Err(e) => {
                    send_or_warn_async(&events, Event::Error {
                        message:     e.to_string(),
                        recoverable: true,
                        session,
                    }, "extended metadata error").await;
                }
            }

            let _ = active.remove_async(&session).await;
        });
    }

    // ── Cancellation helpers ──────────────────────────────────────────────────

    /// Cancel any in-flight operation for `session` and register a fresh token.
    fn arm_cancel(&self, session: SessionId) -> CancellationToken {
        if let Some((_, old)) = self.active.remove_sync(&session) {
            old.cancel();
        }
        let token = CancellationToken::new();
        let _ = self.active.insert_sync(session, token.clone());
        token
    }

    fn cancel(&self, session: SessionId) {
        if let Some((_, token)) = self.active.remove_sync(&session) {
            token.cancel();
        }
    }
}

impl Actor for Previewer {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(PreviewCommand::Generate { path, options, session }) => {
                    self.dispatch_preview(path, options, session);
                }
                Ok(PreviewCommand::LoadMetadata(node, session)) => {
                    self.dispatch_metadata(node, session);
                }
                Ok(PreviewCommand::LoadExtendedMetadata(node, session)) => {
                    self.dispatch_extended_metadata(node, session);
                }
                Ok(PreviewCommand::Cancel(session)) => {
                    self.cancel(session);
                }
                Ok(PreviewCommand::ClearCache) => {
                    if let Ok(mut c) = self.cache.lock() {
                        c.clear();
                    }
                }
                Err(_) => {
                    // Sender dropped — shut down and cancel all in-flight work.
                    self.active.iter_async(|_, v| { v.cancel(); true }).await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "previewer"
    }
}
