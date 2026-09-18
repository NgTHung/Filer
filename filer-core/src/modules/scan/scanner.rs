use flume::Receiver;
use rapidhash::fast::RandomState;
use std::sync::Arc;

use crate::actors::cancel::CancelMap;
use crate::actors::{Actor, WorkTracker};
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::PipelineConfig;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn;
use crate::vfs::provider::FsProvider;

use super::execution::{
    ScanEvents, ScanResources, ScanTarget, scan_directory, scan_segmented_location,
};
use super::paging::PagingSessions;

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
            scan_directory(
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
