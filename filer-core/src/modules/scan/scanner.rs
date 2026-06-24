use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::errors::{CoreError, ErrorCode};
use crate::model::directory::DirectoryLoadState;
use crate::model::directory::{DirectoryLoadOptions, DirectoryPageResult};
use crate::model::location::{LocationId, LocationRef, LocationRoute};
use crate::model::node::NodeEntry;
use crate::model::node::NodeId;
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::{EntryGroup, GroupedEntries, GroupedNodes};
use crate::pipeline::{Pipeline, PipelineConfig, effective_listing};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;
use crate::vfs::segmented::SegmentedLocationResolver;

use super::paging::{PageLoad, PagingSessions};

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
    RefreshLocation {
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
    paging: PagingSessions,
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
            paging: PagingSessions::new(),
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
            paging: PagingSessions::new(),
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
        parent_location_id: Option<LocationId>,
    ) {
        let provider = self.provider.clone();
        let registry = self.registry.clone();
        let events = self.events_sender.clone();
        let active_scans = self.active_scans.clone();
        let latest_scans = self.latest_scans.clone();
        let cache = self.cache.clone();
        let paging = self.paging.clone();

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
                &paging,
                invalidate_cache,
                parent_location,
                parent_location_id,
            )
            .await;
            active_scans.remove_if_current(session, &cancel).await;
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
                    "scan resolve",
                );
                return;
            }
        };
        let route = location.route();
        let path = match &route {
            LocationRoute::DirectPath { path } => path.clone(),
            LocationRoute::Segmented { .. } => {
                self.dispatch_segmented_location_scan(
                    location,
                    session,
                    pipeline_config,
                    load_options,
                    invalidate_cache,
                    request,
                );
                return;
            }
            LocationRoute::UnsupportedProvider { .. } => {
                let error = route.require_direct_path().unwrap_err();
                send_or_warn(
                    &self.events_sender,
                    Event::from_request_error(error, session, request),
                    "scan route",
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
            Some(location.id()),
        );
    }

    fn dispatch_segmented_location_scan(
        &self,
        location: crate::Location,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        _invalidate_cache: bool,
        request: RequestId,
    ) {
        let provider = self.provider.clone();
        let events = self.events_sender.clone();
        let active_scans = self.active_scans.clone();
        let latest_scans = self.latest_scans.clone();
        let descriptor = location.descriptor().clone();
        let parent = LocationRef::from_location(&location);

        let _ = self.latest_scans.remove_sync(&session);
        let _ = self.latest_scans.insert_sync(session, request);
        let cancel = active_scans.arm(session);
        tokio::spawn(async move {
            Self::scan_segmented_location(
                &provider,
                &events,
                descriptor,
                parent,
                session,
                pipeline_config,
                load_options,
                &cancel,
                request,
                &latest_scans,
            )
            .await;
            active_scans.remove_if_current(session, &cancel).await;
        });
    }

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
        paging: &PagingSessions,
        invalidate_cache: bool,
        parent_location: Option<LocationRef>,
        parent_location_id: Option<LocationId>,
    ) {
        if invalidate_cache {
            paging.clear_session(session);
            if let Some(cache) = cache {
                if let Ok(mut cache) = cache.lock() {
                    tracing::debug!(path = %path.display(), "Invalidating directory cache before scan");
                    if let Some(location_id) = parent_location_id {
                        cache.invalidate_location(location_id);
                    }
                    cache.invalidate(path);
                }
            }
        }

        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Started,
                ProgressPhase::Loading,
                ProgressUnit::Step,
                0,
                None,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;

        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::CacheLookup,
                ProgressUnit::Step,
                0,
                None,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;
        let cache_listing = if load_options.is_paged() {
            effective_listing(&pipeline_config, load_options.listing)
        } else {
            load_options.listing
        };
        let cached_nodes = cache.and_then(|c| {
            let mut cache = c.lock().ok()?;
            if let Some(location_id) = parent_location_id
                && let Some(nodes) = cache.get_location(location_id, cache_listing)
            {
                return Some(nodes);
            }

            let nodes = cache.get(path, cache_listing)?;
            if let Some(location_id) = parent_location_id {
                cache.put_location(
                    location_id,
                    path.to_path_buf(),
                    cache_listing,
                    nodes.clone(),
                );
            }
            Some(nodes)
        });
        if let Some(cached) = cached_nodes {
            tracing::trace!(path = %path.display(), session = %session, "Directory scan served from cache");
            let parent_id = registry.clone().register(path.to_path_buf());
            if let Some(page_request) = load_options.page_request() {
                match paging.load_cached(cached, path, session, page_request, &pipeline_config) {
                    Ok(page) => {
                        registry.clone().register_batch_file_node(&page.entries);
                        Self::emit_page_result(
                            events,
                            latest_scans,
                            registry,
                            path,
                            parent_id,
                            parent_location,
                            session,
                            request,
                            page,
                            &pipeline_config,
                            "scan page result (cached)",
                        )
                        .await;
                    }
                    Err(e) => {
                        if Self::is_latest(latest_scans, session, request) {
                            send_or_warn_async(
                                events,
                                Event::from_request_error(e, session, request),
                                "scan cached page error",
                            )
                            .await;
                        }
                    }
                }
                return;
            }

            registry.clone().register_batch_file_node(&cached);
            let pipeline = Pipeline::from_config(&pipeline_config);
            let (groups, load) = pipeline
                .execute_grouped(cached)
                .limited(load_options.snapshot_limit());
            Self::emit_scan_progress(
                events,
                latest_scans,
                session,
                request,
                ProgressSnapshot::new(
                    ProgressStatus::Running,
                    ProgressPhase::Processing,
                    ProgressUnit::Entry,
                    load.loaded_count,
                    load.total_count,
                    Self::scan_target(path, parent_location.as_ref()),
                ),
            )
            .await;
            if !Self::is_latest(latest_scans, session, request) {
                return;
            }
            Self::emit_scan_progress(
                events,
                latest_scans,
                session,
                request,
                ProgressSnapshot::new(
                    ProgressStatus::Running,
                    ProgressPhase::Emitting,
                    ProgressUnit::Entry,
                    load.loaded_count,
                    load.total_count,
                    Self::scan_target(path, parent_location.as_ref()),
                ),
            )
            .await;
            if let Some(parent) = parent_location {
                send_or_warn_async(
                    events,
                    Event::DirectoryLoaded {
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
                    Event::DirectoryLoadedCompat {
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
            Self::emit_scan_progress(
                events,
                latest_scans,
                session,
                request,
                ProgressSnapshot::new(
                    ProgressStatus::Completed,
                    ProgressPhase::Finalizing,
                    ProgressUnit::Entry,
                    load.loaded_count,
                    load.total_count,
                    Self::scan_target(path, None),
                ),
            )
            .await;
            return;
        }

        tracing::trace!(path = %path.display(), session = %session, "Directory scan cache miss, listing provider");
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Loading,
                ProgressUnit::Entry,
                0,
                None,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;

        let cx = ProviderCx::with_cancel(cancel);

        if let Some(page_request) = load_options.page_request() {
            let first_page = page_request.cursor.is_none();
            let page = match paging
                .load_provider(
                    provider.as_ref(),
                    path,
                    session,
                    page_request,
                    &pipeline_config,
                    &cx,
                )
                .await
            {
                Ok(PageLoad::Page(page)) => page,
                Ok(PageLoad::Cancelled) => {
                    Self::emit_scan_progress(
                        events,
                        latest_scans,
                        session,
                        request,
                        ProgressSnapshot::new(
                            ProgressStatus::Cancelled,
                            ProgressPhase::Loading,
                            ProgressUnit::Entry,
                            0,
                            None,
                            Self::scan_target(path, parent_location.as_ref()),
                        ),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    if Self::is_latest(latest_scans, session, request) {
                        Self::emit_scan_progress(
                            events,
                            latest_scans,
                            session,
                            request,
                            ProgressSnapshot::new(
                                ProgressStatus::Failed,
                                ProgressPhase::Loading,
                                ProgressUnit::Entry,
                                0,
                                None,
                                Self::scan_target(path, parent_location.as_ref()),
                            ),
                        )
                        .await;
                        send_or_warn_async(
                            events,
                            Event::from_request_error(e, session, request),
                            "scan page error",
                        )
                        .await;
                    }
                    return;
                }
            };

            if first_page
                && page.state.complete
                && pipeline_config == PipelineConfig::default()
                && let Some(cache) = cache
            {
                if let Ok(mut c) = cache.lock() {
                    if let Some(location_id) = parent_location_id {
                        c.put_location(
                            location_id,
                            path.to_path_buf(),
                            load_options.listing,
                            page.entries.clone(),
                        );
                    } else {
                        c.put(
                            path.to_path_buf(),
                            load_options.listing,
                            page.entries.clone(),
                        );
                    }
                }
            }

            if cancel.is_cancelled() {
                Self::emit_scan_progress(
                    events,
                    latest_scans,
                    session,
                    request,
                    ProgressSnapshot::new(
                        ProgressStatus::Cancelled,
                        ProgressPhase::Loading,
                        ProgressUnit::Entry,
                        0,
                        None,
                        Self::scan_target(path, parent_location.as_ref()),
                    ),
                )
                .await;
                return;
            }

            let parent_id = registry.clone().register(path.to_path_buf());
            registry.clone().register_batch_file_node(&page.entries);
            Self::emit_page_result(
                events,
                latest_scans,
                registry,
                path,
                parent_id,
                parent_location,
                session,
                request,
                page,
                &pipeline_config,
                "scan page result",
            )
            .await;
            return;
        }

        let entries = match cx
            .race(
                provider.scheme(),
                provider.list_with_options(path, load_options.listing, &cx),
            )
            .await
        {
            Ok(entries) => entries,
            Err(e) if e.code() == ErrorCode::Cancelled => {
                Self::emit_scan_progress(
                    events,
                    latest_scans,
                    session,
                    request,
                    ProgressSnapshot::new(
                        ProgressStatus::Cancelled,
                        ProgressPhase::Loading,
                        ProgressUnit::Entry,
                        0,
                        None,
                        Self::scan_target(path, parent_location.as_ref()),
                    ),
                )
                .await;
                return;
            }
            Err(e) => {
                if Self::is_latest(latest_scans, session, request) {
                    Self::emit_scan_progress(
                        events,
                        latest_scans,
                        session,
                        request,
                        ProgressSnapshot::new(
                            ProgressStatus::Failed,
                            ProgressPhase::Loading,
                            ProgressUnit::Entry,
                            0,
                            None,
                            Self::scan_target(path, parent_location.as_ref()),
                        ),
                    )
                    .await;
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

        if !load_options.is_bounded()
            && let Some(cache) = cache
        {
            if let Ok(mut c) = cache.lock() {
                if let Some(location_id) = parent_location_id {
                    c.put_location(
                        location_id,
                        path.to_path_buf(),
                        load_options.listing,
                        entries.clone(),
                    );
                } else {
                    c.put(path.to_path_buf(), load_options.listing, entries.clone());
                }
                tracing::trace!(path = %path.display(), session = %session, count = entries.len(), "Directory scan cached provider listing");
            }
        }

        if cancel.is_cancelled() {
            Self::emit_scan_progress(
                events,
                latest_scans,
                session,
                request,
                ProgressSnapshot::new(
                    ProgressStatus::Cancelled,
                    ProgressPhase::Loading,
                    ProgressUnit::Entry,
                    0,
                    None,
                    Self::scan_target(path, parent_location.as_ref()),
                ),
            )
            .await;
            return;
        }

        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Registering,
                ProgressUnit::Entry,
                entries.len(),
                Some(entries.len()),
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;
        let parent_id = registry.clone().register(path.to_path_buf());
        registry.clone().register_batch_file_node(&entries);

        let pipeline = Pipeline::from_config(&pipeline_config);
        let (groups, load) = pipeline
            .execute_grouped(entries)
            .limited(load_options.snapshot_limit());
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Processing,
                ProgressUnit::Entry,
                load.loaded_count,
                load.total_count,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;

        if cancel.is_cancelled() {
            Self::emit_scan_progress(
                events,
                latest_scans,
                session,
                request,
                ProgressSnapshot::new(
                    ProgressStatus::Cancelled,
                    ProgressPhase::Processing,
                    ProgressUnit::Entry,
                    load.loaded_count,
                    load.total_count,
                    Self::scan_target(path, parent_location.as_ref()),
                ),
            )
            .await;
            return;
        }
        if !Self::is_latest(latest_scans, session, request) {
            return;
        }

        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Emitting,
                ProgressUnit::Entry,
                load.loaded_count,
                load.total_count,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;
        if let Some(parent) = parent_location {
            send_or_warn_async(
                events,
                Event::DirectoryLoaded {
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
                Event::DirectoryLoadedCompat {
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
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Completed,
                ProgressPhase::Finalizing,
                ProgressUnit::Entry,
                load.loaded_count,
                load.total_count,
                Self::scan_target(path, None),
            ),
        )
        .await;
    }

    async fn scan_segmented_location(
        provider: &Arc<dyn FsProvider>,
        events: &Sender<Event>,
        descriptor: crate::LocationDescriptor,
        parent: LocationRef,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        cancel: &CancellationToken,
        request: RequestId,
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
    ) {
        let target_path = descriptor.display_path();
        let target = std::path::Path::new(&target_path);
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Started,
                ProgressPhase::Loading,
                ProgressUnit::Step,
                0,
                None,
                Self::scan_target(target, Some(&parent)),
            ),
        )
        .await;

        let cx = ProviderCx::with_cancel(cancel);
        let entries = match cx
            .race(
                provider.scheme(),
                SegmentedLocationResolver::new(provider.as_ref()).list(&descriptor, &cx),
            )
            .await
        {
            Ok(entries) => entries,
            Err(e) if e.code() == ErrorCode::Cancelled => {
                Self::emit_scan_progress(
                    events,
                    latest_scans,
                    session,
                    request,
                    ProgressSnapshot::new(
                        ProgressStatus::Cancelled,
                        ProgressPhase::Loading,
                        ProgressUnit::Entry,
                        0,
                        None,
                        Self::scan_target(target, Some(&parent)),
                    ),
                )
                .await;
                return;
            }
            Err(e) => {
                if Self::is_latest(latest_scans, session, request) {
                    Self::emit_scan_progress(
                        events,
                        latest_scans,
                        session,
                        request,
                        ProgressSnapshot::new(
                            ProgressStatus::Failed,
                            ProgressPhase::Loading,
                            ProgressUnit::Entry,
                            0,
                            None,
                            Self::scan_target(target, Some(&parent)),
                        ),
                    )
                    .await;
                    send_or_warn_async(
                        events,
                        Event::from_request_error(e, session, request),
                        "scan segmented error",
                    )
                    .await;
                }
                return;
            }
        };

        if cancel.is_cancelled() || !Self::is_latest(latest_scans, session, request) {
            return;
        }

        let groups = grouped_entries(entries, &pipeline_config);
        let (groups, load) = limited_entries(groups, load_options.snapshot_limit());
        send_or_warn_async(
            events,
            Event::DirectoryLoaded {
                parent,
                groups,
                load,
                session,
                request,
            },
            "scan segmented result",
        )
        .await;
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Completed,
                ProgressPhase::Finalizing,
                ProgressUnit::Entry,
                load.loaded_count,
                load.total_count,
                Self::scan_target(target, None),
            ),
        )
        .await;
    }

    async fn emit_page_result(
        events: &Sender<Event>,
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
        registry: &NodeRegistry,
        path: &Path,
        parent_id: NodeId,
        parent_location: Option<LocationRef>,
        session: SessionId,
        request: RequestId,
        page: DirectoryPageResult,
        pipeline_config: &PipelineConfig,
        context: &'static str,
    ) {
        let page_state = page.state.clone();
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Processing,
                ProgressUnit::Entry,
                page_state.page_count,
                page_state.total_count,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;
        if !Self::is_latest(latest_scans, session, request) {
            return;
        }

        let groups = Pipeline::from_config(pipeline_config).execute_grouped(page.entries);
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Running,
                ProgressPhase::Emitting,
                ProgressUnit::Entry,
                page_state.page_count,
                page_state.total_count,
                Self::scan_target(path, parent_location.as_ref()),
            ),
        )
        .await;
        if let Some(parent) = parent_location {
            send_or_warn_async(
                events,
                Event::DirectoryPageLoaded {
                    parent,
                    groups: crate::pipeline::GroupedEntries::from_grouped_nodes(groups, registry),
                    page: page_state.clone(),
                    session,
                    request,
                },
                context,
            )
            .await;
        } else {
            send_or_warn_async(
                events,
                Event::DirectoryPageLoadedCompat {
                    parent: parent_id,
                    path: path.to_path_buf(),
                    groups,
                    page: page_state.clone(),
                    session,
                    request,
                },
                context,
            )
            .await;
        }
        Self::emit_scan_progress(
            events,
            latest_scans,
            session,
            request,
            ProgressSnapshot::new(
                ProgressStatus::Completed,
                ProgressPhase::Finalizing,
                ProgressUnit::Entry,
                page_state.page_count,
                page_state.total_count,
                Self::scan_target(path, None),
            ),
        )
        .await;
    }

    async fn emit_scan_progress(
        events: &Sender<Event>,
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
        session: SessionId,
        request: RequestId,
        snapshot: ProgressSnapshot,
    ) {
        if !Self::is_latest(latest_scans, session, request) {
            return;
        }
        send_or_warn_async(
            events,
            Event::ProgressUpdated {
                scope: ProgressScope::scan(session, request),
                snapshot,
            },
            "scan progress",
        )
        .await;
    }

    fn scan_target(path: &Path, location: Option<&LocationRef>) -> Option<ProgressTarget> {
        location
            .cloned()
            .map(ProgressTarget::Location)
            .or_else(|| Some(ProgressTarget::Path(path.to_path_buf())))
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
        self.paging.clear_session(session);
    }
}

fn grouped_entries(entries: Vec<NodeEntry>, pipeline_config: &PipelineConfig) -> GroupedEntries {
    let mut by_node = HashMap::<NodeId, VecDeque<NodeEntry>>::new();
    let nodes = entries
        .into_iter()
        .map(|entry| {
            let node = entry.to_file_node();
            by_node.entry(entry.id).or_default().push_back(entry);
            node
        })
        .collect();
    let grouped = Pipeline::from_config(pipeline_config).execute_grouped(nodes);
    entries_from_grouped_nodes(grouped, by_node)
}

fn entries_from_grouped_nodes(
    grouped: GroupedNodes,
    mut by_node: HashMap<NodeId, VecDeque<NodeEntry>>,
) -> GroupedEntries {
    let total_count = grouped.total_count;
    GroupedEntries {
        groups: grouped
            .groups
            .into_iter()
            .map(|group| EntryGroup {
                label: group.label,
                nodes: group
                    .nodes
                    .into_iter()
                    .filter_map(|node| by_node.get_mut(&node.id).and_then(VecDeque::pop_front))
                    .collect(),
                order: group.order,
            })
            .collect(),
        total_count,
    }
}

fn limited_entries(
    mut grouped: GroupedEntries,
    limit: Option<usize>,
) -> (GroupedEntries, DirectoryLoadState) {
    let total_count = grouped.total_count;
    let Some(limit) = limit else {
        return (grouped, DirectoryLoadState::complete(total_count));
    };

    let mut remaining = limit;
    let mut loaded_count = 0;
    let mut groups = Vec::new();
    for mut group in grouped.groups {
        if remaining == 0 {
            break;
        }
        if group.nodes.len() > remaining {
            group.nodes.truncate(remaining);
        }
        let group_count = group.nodes.len();
        if group_count > 0 {
            loaded_count += group_count;
            remaining -= group_count;
            groups.push(group);
        }
    }
    grouped.groups = groups;
    grouped.total_count = loaded_count;
    (
        grouped,
        DirectoryLoadState::from_counts(loaded_count, total_count),
    )
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
                Ok(ScanCommand::RefreshLocation {
                    location,
                    session,
                    pipeline,
                    load,
                    request,
                }) => {
                    self.dispatch_location_scan(location, session, pipeline, load, true, request);
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
