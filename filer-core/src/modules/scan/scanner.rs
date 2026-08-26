use flume::Receiver;
use rapidhash::fast::RandomState;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::errors::ErrorCode;
use crate::model::directory::DirectoryLoadState;
use crate::model::directory::{DirectoryLoadOptions, DirectoryPageResult};
use crate::model::location::{Location, LocationId, LocationRef, LocationRoute};
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::GroupedEntries;
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
    Cancel(SessionId),
    Shutdown,
}

/// Scanner actor - handles directory traversal
pub struct Scanner {
    commands: Receiver<ScanCommand>,
    events_sender: EventSink,
    provider: Arc<dyn FsProvider>,
    registry: NodeRegistry,
    active_scans: CancelMap,
    latest_scans: Arc<scc::HashMap<SessionId, RequestId, RandomState>>,
    cache: Option<SharedDirCache>,
    paging: PagingSessions,
    work: WorkTracker,
}

impl Scanner {
    pub fn new<E: Into<EventSink>>(
        commands: Receiver<ScanCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events_sender: events.into(),
            provider,
            registry,
            active_scans: CancelMap::new(),
            latest_scans: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            cache: None,
            paging: PagingSessions::new(),
            work: WorkTracker::new(),
        }
    }

    pub fn with_cache<E: Into<EventSink>>(
        commands: Receiver<ScanCommand>,
        events: E,
        provider: Arc<dyn FsProvider>,
        registry: NodeRegistry,
        cache: SharedDirCache,
    ) -> Self {
        Self {
            commands,
            events_sender: events.into(),
            provider,
            registry,
            active_scans: CancelMap::new(),
            latest_scans: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            cache: Some(cache),
            paging: PagingSessions::new(),
            work: WorkTracker::new(),
        }
    }

    pub(crate) fn with_work_tracker(mut self, work: WorkTracker) -> Self {
        self.work = work;
        self
    }

    fn dispatch_scan_with_location(
        &self,
        path: PathBuf,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
        request: RequestId,
        parent_location: LocationRef,
        parent_location_id: Option<LocationId>,
    ) {
        let provider = self.provider.clone();
        let events = self.events_sender.clone();
        let active_scans = self.active_scans.clone();
        let latest_scans = self.latest_scans.clone();
        let cache = self.cache.clone();
        let paging = self.paging.clone();
        let work = self.work.clone();

        let _ = self.latest_scans.remove_sync(&session);
        let _ = self.latest_scans.insert_sync(session, request);
        let cancel = active_scans.arm(session);
        work.spawn(cancel.clone(), async move {
            Self::scan_directory(
                &provider,
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
                let error = match route.require_direct_path() {
                    Ok(_) => return,
                    Err(error) => error,
                };
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
            LocationRef::from_location(&location),
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
        let work = self.work.clone();

        let _ = self.latest_scans.remove_sync(&session);
        let _ = self.latest_scans.insert_sync(session, request);
        let cancel = active_scans.arm(session);
        work.spawn(cancel.clone(), async move {
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
        events: &EventSink,
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
        parent_location: LocationRef,
        parent_location_id: Option<LocationId>,
    ) {
        if invalidate_cache {
            paging.clear_session(session);
            if let Some(cache) = cache
                && let Ok(mut cache) = cache.lock()
            {
                tracing::debug!(path = %path.display(), "Invalidating directory cache before scan");
                if let Some(location_id) = parent_location_id {
                    cache.invalidate(location_id);
                } else {
                    cache.invalidate_local_subtree(path);
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
                Self::scan_target(path, Some(&parent_location)),
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
        let cache_listing = if load_options.is_paged() {
            effective_listing(&pipeline_config, load_options.listing)
        } else {
            load_options.listing
        };
        let cx = ProviderCx::with_cancel(cancel);
        let cached_nodes = cache.and_then(|c| {
            let mut cache = c.lock().ok()?;
            let location_id = parent_location_id?;
            cache.get(location_id, cache_listing)
        });
        if let Some(cached) = cached_nodes {
            tracing::trace!(path = %path.display(), session = %session, "Directory scan served from cache");
            if let Some(page_request) = load_options.page_request() {
                match paging.load_cached(cached, path, session, page_request, &pipeline_config, &cx)
                {
                    Ok(PageLoad::Page(page)) => {
                        Self::emit_page_result(
                            events,
                            latest_scans,
                            path,
                            parent_location.clone(),
                            session,
                            request,
                            page,
                            &pipeline_config,
                            "scan page result (cached)",
                        )
                        .await;
                    }
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
                                Self::scan_target(path, Some(&parent_location)),
                            ),
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
                    Self::scan_target(path, Some(&parent_location)),
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
                    Self::scan_target(path, Some(&parent_location)),
                ),
            )
            .await;
            send_or_warn_async(
                events,
                Event::DirectoryLoaded {
                    parent: parent_location.clone(),
                    groups,
                    load,
                    session,
                    request,
                },
                "scan location result (cached)",
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
                    Self::scan_target(path, Some(&parent_location)),
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;

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
                            Self::scan_target(path, Some(&parent_location)),
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
                                Self::scan_target(path, Some(&parent_location)),
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
                && let Ok(mut c) = cache.lock()
                && parent_location_id.is_some()
            {
                c.put(
                    cache_location(&parent_location, path),
                    load_options.listing,
                    page.entries.clone(),
                );
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
                        Self::scan_target(path, Some(&parent_location)),
                    ),
                )
                .await;
                return;
            }

            Self::emit_page_result(
                events,
                latest_scans,
                path,
                parent_location.clone(),
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
                        Self::scan_target(path, Some(&parent_location)),
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
                            Self::scan_target(path, Some(&parent_location)),
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
            && let Ok(mut c) = cache.lock()
            && parent_location_id.is_some()
        {
            c.put(
                cache_location(&parent_location, path),
                load_options.listing,
                entries.clone(),
            );
            tracing::trace!(path = %path.display(), session = %session, count = entries.len(), "Directory scan cached provider listing");
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
                    Self::scan_target(path, Some(&parent_location)),
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
        let groups = Pipeline::from_config(&pipeline_config).execute_grouped(entries);
        let (groups, load) = limited_entries(groups, load_options.snapshot_limit());
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
                Self::scan_target(path, Some(&parent_location)),
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
                    Self::scan_target(path, Some(&parent_location)),
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
        send_or_warn_async(
            events,
            Event::DirectoryLoaded {
                parent: parent_location.clone(),
                groups,
                load,
                session,
                request,
            },
            "scan location result",
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
    }

    async fn scan_segmented_location(
        provider: &Arc<dyn FsProvider>,
        events: &EventSink,
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

        let groups = Pipeline::from_config(&pipeline_config).execute_grouped(entries);
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
        events: &EventSink,
        latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
        path: &Path,
        parent_location: LocationRef,
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
                Self::scan_target(path, Some(&parent_location)),
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
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
        send_or_warn_async(
            events,
            Event::DirectoryPageLoaded {
                parent: parent_location.clone(),
                groups,
                page: page_state.clone(),
                session,
                request,
            },
            context,
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
                page_state.page_count,
                page_state.total_count,
                Self::scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
    }

    async fn emit_scan_progress(
        events: &EventSink,
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

fn cache_location(parent: &LocationRef, path: &Path) -> Location {
    parent
        .descriptor()
        .cloned()
        .map(Location::new)
        .unwrap_or_else(|| Location::local(path.to_path_buf()))
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
