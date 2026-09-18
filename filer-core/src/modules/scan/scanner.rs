use flume::Receiver;
use rapidhash::fast::RandomState;
use std::sync::Arc;

use crate::actors::cancel::CancelMap;
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

use super::execution::{
    CacheScan, FullScan, ScanEvents, ScanResources, ScanTarget, emit_page_result,
    emit_scan_progress, invalidate_cache as invalidate_scan_cache, is_latest, scan_cached,
    scan_full, scan_segmented_location, scan_target, store_snapshot,
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
        let cache_scan = CacheScan {
            cache,
            paging,
            cancel,
            events: scan_events,
            path,
            parent_location: &parent_location,
            parent_location_id,
            pipeline_config: &pipeline_config,
            load_options: &load_options,
        };
        if invalidate_cache {
            invalidate_scan_cache(&cache_scan);
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
        let cx = ProviderCx::with_cancel(cancel);
        if scan_cached(&cache_scan).await {
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

            if first_page && page.state.complete && pipeline_config == PipelineConfig::default() {
                store_snapshot(
                    cache,
                    &parent_location,
                    parent_location_id,
                    path,
                    load_options.listing,
                    &page.entries,
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

        scan_full(FullScan {
            provider: provider.as_ref(),
            cancel,
            cache,
            events: scan_events,
            path,
            parent_location: &parent_location,
            parent_location_id,
            pipeline_config: &pipeline_config,
            load_options: &load_options,
        })
        .await;
    }

    fn cancel_scan(&self, session: SessionId) {
        self.active_scans.cancel(session);
        self.paging.clear_session(session);
    }
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
