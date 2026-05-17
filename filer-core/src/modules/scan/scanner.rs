use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::node::NodeId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::provider::FsProvider;

/// Commands for scanner actor
#[derive(Debug, Clone)]
pub enum ScanCommand {
    Scan {
        path: PathBuf,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    ScanLocation {
        location: LocationRef,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    ScanNode {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    RefreshNode {
        node: NodeId,
        session: SessionId,
        pipeline: PipelineConfig,
        load: DirectoryLoadOptions,
        request: RequestId,
    },
    Cancel(SessionId),
    Shutdown,
}

/// Scanner actor - handles directory traversal
pub struct Scanner {
    commands: Receiver<ScanCommand>,
    events_sender: Sender<Event>,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_scans: CancelMap,
    latest_scans: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
    cache: Option<SharedDirCache>,
}

impl Scanner {
    pub fn new(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events_sender: events,
            provider,
            registry,
            active_scans: CancelMap::new(),
            latest_scans: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            cache: None,
        }
    }

    pub fn with_cache(
        commands: Receiver<ScanCommand>,
        events: Sender<Event>,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        Self {
            commands,
            events_sender: events,
            provider,
            registry,
            active_scans: CancelMap::new(),
            latest_scans: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            cache: Some(cache),
        }
    }

    /// Resolve a scan command to a concrete path, then spawn the scan task.
    ///
    /// Both `Scan` (raw path) and `ScanNode` (NodeId) funnel through here
    /// after resolving to a `PathBuf`.  NodeId resolution is cheap
    /// (in-memory registry lookup) and happens *before* the spawn so we
    /// fail fast without creating cancellation tokens for invalid nodes.
    fn dispatch_scan(
        &self,
        path: PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
        request: RequestId,
    ) {
        self.dispatch_scan_with_location(
            path,
            session,
            pipeline_config,
            load_options,
            invalidate_cache,
            request,
            None,
        );
    }

    fn dispatch_scan_with_location(
        &self,
        path: PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
        request: RequestId,
        parent_location: Option<LocationRef>,
    ) {
        let provider = self.provider.clone();
        let registry = self.registry.clone();
        let events = self.events_sender.clone();
        let active_scans = self.active_scans.clone();
        let latest_scans = self.latest_scans.clone();
        let cache = self.cache.clone();

        let _ = self.latest_scans.remove_sync(&session);
        let _ = self.latest_scans.insert_sync(session, request);
        let cancel = active_scans.arm(session);
        tokio::spawn(async move {
            Self::scan_directory(
                &provider,
                &registry,
                &events,
                &path,
                session,
                pipeline_config,
                load_options,
                &cancel,
                request,
                &latest_scans,
                cache.as_ref(),
                invalidate_cache,
                parent_location,
            )
            .await;
            active_scans.remove(session).await;
        });
    }

    fn dispatch_location_scan(
        &self,
        location_ref: LocationRef,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
        request: RequestId,
    ) {
        let location = match self.registry.resolve_location_ref(&location_ref) {
            Ok(location) => location,
            Err(error) => {
                send_or_warn(
                    &self.events_sender,
                    Event::from_request_error(error, session, request),
                    "scan.location resolve",
                );
                return;
            }
        };
        let route = location.route();
        let path = match &route {
            LocationRoute::DirectPath { path } => path.clone(),
            LocationRoute::Segmented { .. } | LocationRoute::UnsupportedProvider { .. } => {
                let error = route.require_direct_path().unwrap_err();
                send_or_warn(
                    &self.events_sender,
                    Event::from_request_error(error, session, request),
                    "scan.location route",
                );
                return;
            }
        };
        self.dispatch_scan_with_location(
            path,
            session,
            pipeline_config,
            load_options,
            invalidate_cache,
            request,
            Some(LocationRef::from_location(&location)),
        );
    }

    /// Perform a single directory scan: list → register → pipeline → emit.
    ///
    /// Checks the cancellation token at two points:
    /// 1. After the (potentially slow) filesystem listing
    /// 2. After pipeline execution, before sending the result
    async fn scan_directory(
        provider: &Arc<dyn FsProvider>,
        registry: &NodeRegistry,
        events: &Sender<Event>,
        path: &Path,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        cancel: &CancellationToken,
        request: RequestId,
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
        cache: Option<&SharedDirCache>,
        invalidate_cache: bool,
        parent_location: Option<LocationRef>,
    ) {
        if invalidate_cache {
            if let Some(cache) = cache {
                if let Ok(mut cache) = cache.lock() {
                    tracing::debug!(path = %path.display(), "Invalidating directory cache before scan");
                    cache.invalidate(path);
                }
            }
        }

        // 1. Cache check (before I/O)
        let cached_nodes = cache.and_then(|c| c.lock().ok()?.get(path, load_options.listing));
        if let Some(cached) = cached_nodes {
            tracing::trace!(path = %path.display(), session = %session, "Directory scan served from cache");
            let parent_id = registry.clone().register(path.to_path_buf());
            registry.clone().register_batch_file_node(&cached);
            let pipeline = Pipeline::from_config(&pipeline_config);
            let (groups, load) = pipeline.execute_grouped(cached).limited(load_options.limit);
            if !Self::is_latest(latest_scans, session, request) {
                return;
            }
            if let Some(parent) = parent_location {
                send_or_warn_async(
                    events,
                    Event::DirectoryEntriesLoaded {
                        parent,
                        groups: crate::pipeline::GroupedEntries::from_grouped_nodes(
                            groups, registry,
                        ),
                        load,
                        session,
                        request,
                    },
                    "scan location result (cached)",
                )
                .await;
            } else {
                send_or_warn_async(
                    events,
                    Event::DirectoryLoaded {
                        parent: parent_id,
                        path: path.to_path_buf(),
                        groups,
                        load,
                        session,
                        request,
                    },
                    "scan result (cached)",
                )
                .await;
            }
            return;
        }

        // 2. List directory (cache miss)
        tracing::trace!(path = %path.display(), session = %session, "Directory scan cache miss, listing provider");
        let entries = match provider.list_with_options(path, load_options.listing).await {
            Ok(entries) => entries,
            Err(e) => {
                if Self::is_latest(latest_scans, session, request) {
                    send_or_warn_async(
                        events,
                        Event::from_request_error(e, session, request),
                        "scan error",
                    )
                    .await;
                }
                return;
            }
        };

        // Populate cache after successful unbounded lists.
        if !load_options.is_bounded()
            && let Some(cache) = cache
        {
            if let Ok(mut c) = cache.lock() {
                c.put(path.to_path_buf(), load_options.listing, entries.clone());
                tracing::trace!(path = %path.display(), session = %session, count = entries.len(), "Directory scan cached provider listing");
            }
        }

        // 3. Check cancellation after I/O
        if cancel.is_cancelled() {
            return;
        }

        // 4. Register nodes
        let parent_id = registry.clone().register(path.to_path_buf());
        registry.clone().register_batch_file_node(&entries);

        // 5. Execute pipeline (always returns GroupedNodes)
        let pipeline = Pipeline::from_config(&pipeline_config);
        let (groups, load) = pipeline
            .execute_grouped(entries)
            .limited(load_options.limit);

        // 5. Check cancellation after pipeline
        if cancel.is_cancelled() {
            return;
        }
        if !Self::is_latest(latest_scans, session, request) {
            return;
        }

        // 6. Send result
        if let Some(parent) = parent_location {
            send_or_warn_async(
                events,
                Event::DirectoryEntriesLoaded {
                    parent,
                    groups: crate::pipeline::GroupedEntries::from_grouped_nodes(groups, registry),
                    load,
                    session,
                    request,
                },
                "scan location result",
            )
            .await;
        } else {
            send_or_warn_async(
                events,
                Event::DirectoryLoaded {
                    parent: parent_id,
                    path: path.to_path_buf(),
                    groups,
                    load,
                    session,
                    request,
                },
                "scan result",
            )
            .await;
        }
    }

    fn is_latest(
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
        session: SessionId,
        request: RequestId,
    ) -> bool {
        latest_scans
            .read_sync(&session, |_, latest| *latest == request)
            .unwrap_or(false)
    }

    fn cancel_scan(&self, session: SessionId) {
        self.active_scans.cancel(session);
    }
}

impl Actor for Scanner {
    async fn run(self) {
        loop {
            match self.commands.recv_async().await {
                Ok(ScanCommand::Scan {
                    path,
                    session,
                    pipeline,
                    load,
                    request,
                }) => {
                    self.dispatch_scan(path, session, pipeline, load, false, request);
                }
                Ok(ScanCommand::ScanLocation {
                    location,
                    session,
                    pipeline,
                    load,
                    request,
                }) => {
                    self.dispatch_location_scan(location, session, pipeline, load, false, request);
                }
                Ok(ScanCommand::ScanNode {
                    node,
                    session,
                    pipeline,
                    load,
                    request,
                }) => {
                    // Resolve NodeId → PathBuf before spawning.
                    // Cheap in-memory lookup; fail fast if invalid.
                    let Some(path) = self.registry.resolve(node) else {
                        send_or_warn(
                            &self.events_sender,
                            Event::from_request_error(
                                CoreError::invalid_input(format!("Unable to resolve ID: {node:?}")),
                                session,
                                request,
                            ),
                            "scan resolve error",
                        );
                        continue;
                    };
                    self.dispatch_scan(path, session, pipeline, load, false, request);
                }
                Ok(ScanCommand::RefreshNode {
                    node,
                    session,
                    pipeline,
                    load,
                    request,
                }) => {
                    let Some(path) = self.registry.resolve(node) else {
                        send_or_warn(
                            &self.events_sender,
                            Event::from_request_error(
                                CoreError::invalid_input(format!("Unable to resolve ID: {node:?}")),
                                session,
                                request,
                            ),
                            "scan refresh resolve error",
                        );
                        continue;
                    };
                    self.dispatch_scan(path, session, pipeline, load, true, request);
                }
                Ok(ScanCommand::Cancel(session)) => {
                    self.cancel_scan(session);
                }
                Err(_) | Ok(ScanCommand::Shutdown) => {
                    self.active_scans.cancel_all().await;
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "scanner"
    }
}
