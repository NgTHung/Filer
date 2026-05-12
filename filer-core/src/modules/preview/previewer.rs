use std::sync::{Arc, Mutex};
use std::time::Duration;

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::model::node::{FileNode, NodeId};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
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
        request: RequestId,
    },
    /// Load basic metadata (NodeMeta) for a file
    LoadMetadata(NodeId, SessionId, RequestId),
    /// Load extended metadata (EXIF, ID3, page count…) for a file
    LoadExtendedMetadata(NodeId, SessionId, RequestId),
    /// Cancel all ongoing work for a session
    Cancel(SessionId),
    /// Drop all cached previews
    ClearCache,
}

/// Previewer actor — generates file previews and extracts metadata.
///
/// Each session can have at most one in-flight operation (preview generation
/// or extended-metadata extraction). Dispatching a new operation for a session
/// cancels the previous one.
pub struct Previewer {
    commands: Receiver<PreviewCommand>,
    events: Sender<Event>,
    preview_registry: Arc<PreviewRegistry>,
    metadata_registry: Arc<MetadataRegistry>,
    cache: Arc<Mutex<PreviewCache>>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active: CancelMap,
    latest: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
}

impl Previewer {
    pub fn new(
        commands: Receiver<PreviewCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events,
            preview_registry: Arc::new(PreviewRegistry::with_defaults()),
            metadata_registry: Arc::new(MetadataRegistry::with_defaults()),
            cache: Arc::new(Mutex::new(PreviewCache::new(
                64 * 1024 * 1024,
                Duration::from_secs(300),
            ))),
            provider,
            registry,
            active: CancelMap::new(),
            latest: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    pub fn with_components(
        commands: Receiver<PreviewCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        preview_registry: Arc<PreviewRegistry>,
        cache: Arc<Mutex<PreviewCache>>,
    ) -> Self {
        Self {
            commands,
            events,
            provider,
            registry,
            preview_registry,
            metadata_registry: Arc::new(MetadataRegistry::with_defaults()),
            cache,
            active: CancelMap::new(),
            latest: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
        }
    }

    // ── Preview generation ────────────────────────────────────────────────────

    fn dispatch_preview(
        &self,
        node: NodeId,
        options: Option<PreviewOptions>,
        session: SessionId,
        request: RequestId,
    ) {
        self.mark_latest(session, request);
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {node:?}"),
                    recoverable: true,
                    session,
                    request: Some(request),
                    operation: None,
                },
                "previewer: resolve",
            );
            return;
        };

        // Check cache first — no spawn needed on hit.
        if let Ok(cache) = self.cache.lock() {
            if let Some(preview) = cache.get(&path) {
                send_or_warn(
                    &self.events,
                    Event::PreviewReady {
                        node,
                        preview,
                        session,
                        request,
                    },
                    "previewer: cache hit",
                );
                return;
            }
        }

        let cancel = self.arm_cancel(session);
        let events = self.events.clone();
        let preview_registry = self.preview_registry.clone();
        let cache = self.cache.clone();
        let active = self.active.clone();
        let latest = self.latest.clone();
        let opts = options.unwrap_or_default();

        tokio::spawn(async move {
            if cancel.is_cancelled() {
                return;
            }

            let result = preview_registry.generate_with_options(&path, &opts).await;

            if cancel.is_cancelled() {
                return;
            }
            if !Self::is_latest(&latest, session, request) {
                return;
            }

            match result {
                Ok(preview) => {
                    // Store in cache before emitting.
                    if let Ok(mut c) = cache.lock() {
                        c.put(path, preview.clone());
                    }
                    send_or_warn_async(
                        &events,
                        Event::PreviewReady {
                            node,
                            preview,
                            session,
                            request,
                        },
                        "preview ready",
                    )
                    .await;
                }
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        Event::PreviewFailed {
                            node,
                            reason: e.to_string(),
                            session,
                            request,
                        },
                        "preview failed",
                    )
                    .await;
                }
            }

            active.remove(session).await;
        });
    }

    // ── Basic metadata ────────────────────────────────────────────────────────

    fn dispatch_metadata(&self, node: NodeId, session: SessionId, request: RequestId) {
        self.mark_latest(session, request);
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {node:?}"),
                    recoverable: true,
                    session,
                    request: Some(request),
                    operation: None,
                },
                "previewer: resolve",
            );
            return;
        };

        let events = self.events.clone();
        let registry = self.registry.clone();
        let latest = self.latest.clone();

        tokio::spawn(async move {
            if !Self::is_latest(&latest, session, request) {
                return;
            }
            match FileNode::from_path(path, Some(registry)) {
                Ok(file_node) => {
                    send_or_warn_async(
                        &events,
                        Event::MetadataLoaded {
                            node,
                            meta: file_node.meta,
                            session,
                            request,
                        },
                        "metadata loaded",
                    )
                    .await;
                }
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        Event::Error {
                            message: e.to_string(),
                            recoverable: true,
                            session,
                            request: Some(request),
                            operation: None,
                        },
                        "metadata error",
                    )
                    .await;
                }
            }
        });
    }

    // ── Extended metadata ─────────────────────────────────────────────────────

    fn dispatch_extended_metadata(&self, node: NodeId, session: SessionId, request: RequestId) {
        self.mark_latest(session, request);
        let Some(path) = self.registry.resolve(node) else {
            send_or_warn(
                &self.events,
                Event::Error {
                    message: format!("Cannot resolve node {node:?}"),
                    recoverable: true,
                    session,
                    request: Some(request),
                    operation: None,
                },
                "previewer: resolve",
            );
            return;
        };

        let cancel = self.arm_cancel(session);
        let events = self.events.clone();
        let metadata_registry = self.metadata_registry.clone();
        let provider = self.provider.clone();
        let active = self.active.clone();
        let latest = self.latest.clone();

        tokio::spawn(async move {
            if cancel.is_cancelled() {
                return;
            }

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

            if cancel.is_cancelled() {
                return;
            }
            if !Self::is_latest(&latest, session, request) {
                return;
            }

            match metadata_registry
                .extract(&path, &mime, provider.as_ref())
                .await
            {
                Ok(extended) => {
                    send_or_warn_async(
                        &events,
                        Event::ExtendedMetadataLoaded {
                            node,
                            extended,
                            session,
                            request,
                        },
                        "extended metadata",
                    )
                    .await;
                }
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        Event::Error {
                            message: e.to_string(),
                            recoverable: true,
                            session,
                            request: Some(request),
                            operation: None,
                        },
                        "extended metadata error",
                    )
                    .await;
                }
            }

            active.remove(session).await;
        });
    }

    // ── Cancellation helpers ──────────────────────────────────────────────────

    fn arm_cancel(&self, session: SessionId) -> CancellationToken {
        self.active.arm(session)
    }

    fn mark_latest(&self, session: SessionId, request: RequestId) {
        let _ = self.latest.remove_sync(&session);
        let _ = self.latest.insert_sync(session, request);
    }

    fn is_latest(
        latest: &scc::HashMap<SessionId, RequestId, RandomState>,
        session: SessionId,
        request: RequestId,
    ) -> bool {
        latest
            .read_sync(&session, |_, latest| *latest == request)
            .unwrap_or(false)
    }

    fn cancel(&self, session: SessionId) {
        self.active.cancel(session);
    }
}

impl Actor for Previewer {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(PreviewCommand::Generate {
                    path,
                    options,
                    session,
                    request,
                }) => {
                    self.dispatch_preview(path, options, session, request);
                }
                Ok(PreviewCommand::LoadMetadata(node, session, request)) => {
                    self.dispatch_metadata(node, session, request);
                }
                Ok(PreviewCommand::LoadExtendedMetadata(node, session, request)) => {
                    self.dispatch_extended_metadata(node, session, request);
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
                    self.active.cancel_all().await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "previewer"
    }
}
