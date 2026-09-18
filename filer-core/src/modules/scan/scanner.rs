use flume::Receiver;
use rapidhash::fast::RandomState;
use std::path::Path;
use std::sync::Arc;

use crate::actors::cancel::CancelMap;
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::errors::ErrorCode;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationRef, LocationRoute};
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig, effective_listing};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

use super::execution::{
    ScanEvents, ScanResources, ScanTarget, emit_page_result, emit_scan_progress, is_latest,
    limited_entries, scan_segmented_location, scan_target,
};
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
        target: ScanTarget,
        session: SessionId,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
        request: RequestId,
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
                ScanResources {
                    provider: &provider,
                    cancel: &cancel,
                    cache: cache.as_ref(),
                    paging: &paging,
                },
                ScanEvents {
                    events: &events,
                    latest_scans: &latest_scans,
                    session,
                    request,
                },
                target,
                pipeline_config,
                load_options,
                invalidate_cache,
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
            ScanTarget {
                path,
                parent_location: LocationRef::from_location(&location),
                parent_location_id: Some(location.id()),
            },
            session,
            pipeline_config,
            load_options,
            invalidate_cache,
            request,
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
            scan_segmented_location(
                &provider,
                ScanEvents {
                    events: &events,
                    latest_scans: &latest_scans,
                    session,
                    request,
                },
                descriptor,
                parent,
                pipeline_config,
                load_options,
                &cancel,
            )
            .await;
            active_scans.remove_if_current(session, &cancel).await;
        });
    }

    async fn scan_directory(
        resources: ScanResources<'_>,
        scan_events: ScanEvents<'_>,
        target: ScanTarget,
        pipeline_config: PipelineConfig,
        load_options: DirectoryLoadOptions,
        invalidate_cache: bool,
    ) {
        let ScanResources {
            provider,
            cancel,
            cache,
            paging,
        } = resources;
        let ScanEvents {
            events,
            latest_scans,
            session,
            request,
        } = scan_events;
        let ScanTarget {
            path,
            parent_location,
            parent_location_id,
        } = target;
        let path = path.as_path();
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

        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
            ),
        )
        .await;

        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
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
                        emit_page_result(
                            scan_events,
                            path,
                            parent_location.clone(),
                            page,
                            &pipeline_config,
                            "scan page result (cached)",
                        )
                        .await;
                    }
                    Ok(PageLoad::Cancelled) => {
                        emit_scan_progress(
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
                                scan_target(path, Some(&parent_location)),
                            ),
                        )
                        .await;
                    }
                    Err(e) => {
                        if is_latest(latest_scans, session, request) {
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
            emit_scan_progress(
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
                    scan_target(path, Some(&parent_location)),
                ),
            )
            .await;
            if !is_latest(latest_scans, session, request) {
                return;
            }
            emit_scan_progress(
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
                    scan_target(path, Some(&parent_location)),
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
            emit_scan_progress(
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
                    scan_target(path, Some(&parent_location)),
                ),
            )
            .await;
            return;
        }

        tracing::trace!(path = %path.display(), session = %session, "Directory scan cache miss, listing provider");
        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
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
                    emit_scan_progress(
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
                            scan_target(path, Some(&parent_location)),
                        ),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    if is_latest(latest_scans, session, request) {
                        emit_scan_progress(
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
                                scan_target(path, Some(&parent_location)),
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
                emit_scan_progress(
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
                        scan_target(path, Some(&parent_location)),
                    ),
                )
                .await;
                return;
            }

            emit_page_result(
                scan_events,
                path,
                parent_location.clone(),
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
                emit_scan_progress(
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
                        scan_target(path, Some(&parent_location)),
                    ),
                )
                .await;
                return;
            }
            Err(e) => {
                if is_latest(latest_scans, session, request) {
                    emit_scan_progress(
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
                            scan_target(path, Some(&parent_location)),
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
            emit_scan_progress(
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
                    scan_target(path, Some(&parent_location)),
                ),
            )
            .await;
            return;
        }

        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
        let groups = Pipeline::from_config(&pipeline_config).execute_grouped(entries);
        let (groups, load) = limited_entries(groups, load_options.snapshot_limit());
        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
            ),
        )
        .await;

        if cancel.is_cancelled() {
            emit_scan_progress(
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
                    scan_target(path, Some(&parent_location)),
                ),
            )
            .await;
            return;
        }
        if !is_latest(latest_scans, session, request) {
            return;
        }

        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
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
        emit_scan_progress(
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
                scan_target(path, Some(&parent_location)),
            ),
        )
        .await;
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
