use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flume::Receiver;
use rapidhash::fast::RandomState;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::services::mime::{MAGIC_BYTE_WINDOW, MimeDetector};
use crate::services::preview::PreviewCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;
use crate::{MetadataRegistry, PreviewOptions, PreviewRegistry};

/// Commands for previewer actor
#[derive(Debug, Clone)]
pub enum PreviewCommand {
    /// Generate preview for a file
    Generate {
        location: LocationRef,
        options: Option<PreviewOptions>,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    },
    /// Load basic metadata (NodeMeta) for a file
    LoadMetadata {
        location: LocationRef,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    },
    /// Load extended metadata (EXIF, ID3, page count…) for a file
    LoadExtendedMetadata {
        location: LocationRef,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    },
    /// Cancel all ongoing work for a session
    Cancel(SessionId),
    /// Drop all cached previews
    ClearCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewEventMode {
    Location,
    Compat { node: NodeId },
}

/// Previewer actor — generates file previews and extracts metadata.
///
/// Each session can have at most one in-flight operation (preview generation
/// or extended-metadata extraction). Dispatching a new operation for a session
/// cancels the previous one.
pub struct Previewer {
    commands: Receiver<PreviewCommand>,
    events: EventSink,
    preview_registry: Arc<PreviewRegistry>,
    metadata_registry: Arc<MetadataRegistry>,
    cache: Arc<Mutex<PreviewCache>>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active: CancelMap,
    latest: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
    work: WorkTracker,
}

impl Previewer {
    pub fn new<E: Into<EventSink>>(
        commands: Receiver<PreviewCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events: events.into(),
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
            work: WorkTracker::new(),
        }
    }

    pub fn with_components<E: Into<EventSink>>(
        commands: Receiver<PreviewCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        preview_registry: Arc<PreviewRegistry>,
        cache: Arc<Mutex<PreviewCache>>,
    ) -> Self {
        Self {
            commands,
            events: events.into(),
            provider,
            registry,
            preview_registry,
            metadata_registry: Arc::new(MetadataRegistry::with_defaults()),
            cache,
            active: CancelMap::new(),
            latest: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            work: WorkTracker::new(),
        }
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    fn dispatch_preview(
        &self,
        location_ref: LocationRef,
        options: Option<PreviewOptions>,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    ) {
        self.mark_latest(session, request);
        let Some((location, path)) =
            self.resolve_location_path(&location_ref, session, request, "previewer: resolve")
        else {
            return;
        };

        if let Ok(cache) = self.cache.lock() {
            if let Some(preview) = cache.get(&path) {
                let event = match event_mode {
                    PreviewEventMode::Location => Event::PreviewReady {
                        location,
                        preview,
                        session,
                        request,
                    },
                    PreviewEventMode::Compat { node } => Event::PreviewReadyCompat {
                        node,
                        preview,
                        session,
                        request,
                    },
                };
                send_or_warn(&self.events, event, "previewer: cache hit");
                return;
            }
        }

        let cancel = self.arm_cancel(session);
        let events = self.events.clone();
        let preview_registry = self.preview_registry.clone();
        let provider = self.provider.clone();
        let cache = self.cache.clone();
        let active = self.active.clone();
        let latest = self.latest.clone();
        let opts = options.unwrap_or_default();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            if cancel.is_cancelled() {
                return;
            }

            let cx = ProviderCx::with_cancel(&cancel);
            let result = preview_registry
                .generate_with_options(&path, &opts, provider.as_ref(), &cx)
                .await;

            if cancel.is_cancelled() {
                return;
            }
            if !Self::is_latest(&latest, session, request) {
                return;
            }

            match result {
                Ok(preview) => {
                    if let Ok(mut c) = cache.lock() {
                        c.put(path, preview.clone());
                    }
                    let event = match event_mode {
                        PreviewEventMode::Location => Event::PreviewReady {
                            location,
                            preview,
                            session,
                            request,
                        },
                        PreviewEventMode::Compat { node } => Event::PreviewReadyCompat {
                            node,
                            preview,
                            session,
                            request,
                        },
                    };
                    send_or_warn_async(&events, event, "preview ready").await;
                }
                Err(e) => {
                    let reason = e.to_string();
                    let event = match event_mode {
                        PreviewEventMode::Location => Event::PreviewFailed {
                            location,
                            reason,
                            session,
                            request,
                        },
                        PreviewEventMode::Compat { node } => Event::PreviewFailedCompat {
                            node,
                            reason,
                            session,
                            request,
                        },
                    };
                    send_or_warn_async(&events, event, "preview failed").await;
                }
            }

            active.remove_if_current(session, &cancel).await;
        });
    }

    fn resolve_location_path(
        &self,
        location_ref: &LocationRef,
        session: SessionId,
        request: RequestId,
        context: &'static str,
    ) -> Option<(LocationRef, PathBuf)> {
        let location = match self.registry.resolve_location_ref(location_ref) {
            Ok(location) => location,
            Err(error) => {
                send_or_warn(
                    &self.events,
                    Event::from_request_error(error, session, request),
                    context,
                );
                return None;
            }
        };
        let location_ref = LocationRef::from_location(&location);
        match location.route() {
            LocationRoute::DirectPath { path } => Some((location_ref, path)),
            route @ (LocationRoute::Segmented { .. }
            | LocationRoute::UnsupportedProvider { .. }) => {
                let error = route.require_direct_path().unwrap_err();
                send_or_warn(
                    &self.events,
                    Event::from_request_error(error, session, request),
                    context,
                );
                None
            }
        }
    }

    fn dispatch_metadata(
        &self,
        location_ref: LocationRef,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    ) {
        self.mark_latest(session, request);
        let Some((location, path)) = self.resolve_location_path(
            &location_ref,
            session,
            request,
            "previewer: resolve metadata",
        ) else {
            return;
        };

        let events = self.events.clone();
        let provider = self.provider.clone();
        let latest = self.latest.clone();
        let cancel = self.arm_cancel(session);
        let active = self.active.clone();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            if cancel.is_cancelled() || !Self::is_latest(&latest, session, request) {
                return;
            }
            let cx = ProviderCx::with_cancel(&cancel);
            match cx
                .race(provider.scheme(), provider.metadata(&path, &cx))
                .await
            {
                Ok(file_node) => {
                    if cancel.is_cancelled() || !Self::is_latest(&latest, session, request) {
                        return;
                    }
                    let event = match event_mode {
                        PreviewEventMode::Location => Event::MetadataLoaded {
                            location,
                            meta: file_node.meta,
                            session,
                            request,
                        },
                        PreviewEventMode::Compat { node } => Event::MetadataLoadedCompat {
                            node,
                            meta: file_node.meta,
                            session,
                            request,
                        },
                    };
                    send_or_warn_async(&events, event, "metadata loaded").await;
                }
                Err(e) => {
                    if cancel.is_cancelled() || !Self::is_latest(&latest, session, request) {
                        return;
                    }
                    send_or_warn_async(
                        &events,
                        Event::from_request_error(e, session, request),
                        "metadata error",
                    )
                    .await;
                }
            }
            active.remove_if_current(session, &cancel).await;
        });
    }

    fn dispatch_extended_metadata(
        &self,
        location_ref: LocationRef,
        event_mode: PreviewEventMode,
        session: SessionId,
        request: RequestId,
    ) {
        self.mark_latest(session, request);
        let Some((location, path)) = self.resolve_location_path(
            &location_ref,
            session,
            request,
            "previewer: resolve extended metadata",
        ) else {
            return;
        };

        let cancel = self.arm_cancel(session);
        let events = self.events.clone();
        let metadata_registry = self.metadata_registry.clone();
        let provider = self.provider.clone();
        let active = self.active.clone();
        let latest = self.latest.clone();
        let work = self.work.clone();

        work.spawn(cancel.clone(), async move {
            if cancel.is_cancelled() {
                return;
            }

            let cx = ProviderCx::with_cancel(&cancel);
            // Tier 1: extension-based MIME detection (zero I/O).
            // Upgrade to magic bytes if extension is ambiguous.
            let mime = {
                let ext_info = MimeDetector::detect_from_path(&path);
                if ext_info.confidence == crate::services::mime::DetectionConfidence::Definitive {
                    ext_info
                } else {
                    let header = provider
                        .read_header(&path, MAGIC_BYTE_WINDOW, &cx)
                        .await
                        .ok();
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
                .extract(&path, &mime, provider.as_ref(), &cx)
                .await
            {
                Ok(extended) => {
                    let event = match event_mode {
                        PreviewEventMode::Location => Event::ExtendedMetadataLoaded {
                            location,
                            extended,
                            session,
                            request,
                        },
                        PreviewEventMode::Compat { node } => Event::ExtendedMetadataLoadedCompat {
                            node,
                            extended,
                            session,
                            request,
                        },
                    };
                    send_or_warn_async(&events, event, "extended metadata").await;
                }
                Err(e) => {
                    send_or_warn_async(
                        &events,
                        Event::from_request_error(e, session, request),
                        "extended metadata error",
                    )
                    .await;
                }
            }

            active.remove_if_current(session, &cancel).await;
        });
    }

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
                    location,
                    options,
                    event_mode,
                    session,
                    request,
                }) => {
                    self.dispatch_preview(location, options, event_mode, session, request);
                }
                Ok(PreviewCommand::LoadMetadata {
                    location,
                    event_mode,
                    session,
                    request,
                }) => {
                    self.dispatch_metadata(location, event_mode, session, request);
                }
                Ok(PreviewCommand::LoadExtendedMetadata {
                    location,
                    event_mode,
                    session,
                    request,
                }) => {
                    self.dispatch_extended_metadata(location, event_mode, session, request);
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
