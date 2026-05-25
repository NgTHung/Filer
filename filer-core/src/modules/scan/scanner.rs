use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::actors::Actor;
use crate::actors::cancel::{CancelMap, CancellationToken};
use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::directory::{
    DirectoryCursor, DirectoryLoadOptions, DirectoryPageRequest, DirectoryPageResult,
    DirectoryPageState,
};
use crate::model::location::{LocationRef, LocationRoute};
use crate::model::node::NodeId;
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::{Pipeline, PipelineConfig, PipelinePagingMode};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::vfs::provider::{FsProvider, ListingOptions, parse_offset_cursor, validate_page_limit};

const FILTER_CURSOR_PREFIX: &str = "filter:v1:";

enum FilteredPageLoad {
    Page(DirectoryPageResult),
    Cancelled,
}

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

        // 1. Cache check (before I/O)
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
        let cached_nodes = cache.and_then(|c| c.lock().ok()?.get(path, load_options.listing));
        if let Some(cached) = cached_nodes {
            tracing::trace!(path = %path.display(), session = %session, "Directory scan served from cache");
            let parent_id = registry.clone().register(path.to_path_buf());
            if let Some(page_request) = load_options.page_request() {
                match pipeline_config.paging_mode() {
                    PipelinePagingMode::ProviderPage | PipelinePagingMode::FilteredPage => {
                        let page = match pipeline_config.paging_mode() {
                            PipelinePagingMode::ProviderPage => {
                                Self::page_from_cached_listing(cached, &page_request)
                            }
                            PipelinePagingMode::FilteredPage => {
                                Self::filtered_page_from_cached_listing(
                                    cached,
                                    &page_request,
                                    &pipeline_config,
                                )
                            }
                            PipelinePagingMode::SnapshotOnly => unreachable!(),
                        };

                        match page {
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
                    PipelinePagingMode::SnapshotOnly => {}
                }
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

        // 2. List directory (cache miss)
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

        if let Some(page_request) = load_options.page_request()
            && !matches!(
                pipeline_config.paging_mode(),
                PipelinePagingMode::SnapshotOnly
            )
        {
            let page = match pipeline_config.paging_mode() {
                PipelinePagingMode::ProviderPage => provider
                    .list_page(path, page_request.clone())
                    .await
                    .map(FilteredPageLoad::Page),
                PipelinePagingMode::FilteredPage => {
                    Self::load_filtered_page(
                        provider.as_ref(),
                        path,
                        page_request.clone(),
                        &pipeline_config,
                        cancel,
                    )
                    .await
                }
                PipelinePagingMode::SnapshotOnly => unreachable!(),
            };

            let page = match page {
                Ok(FilteredPageLoad::Page(page)) => page,
                Ok(FilteredPageLoad::Cancelled) => {
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

            if page_request.cursor.is_none()
                && page.state.complete
                && matches!(
                    pipeline_config.paging_mode(),
                    PipelinePagingMode::ProviderPage
                )
                && let Some(cache) = cache
            {
                if let Ok(mut c) = cache.lock() {
                    c.put(
                        path.to_path_buf(),
                        load_options.listing,
                        page.entries.clone(),
                    );
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
                "scan page result",
            )
            .await;
            return;
        }

        let entries = match provider.list_with_options(path, load_options.listing).await {
            Ok(entries) => entries,
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

        // 4. Register nodes
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

        // 5. Execute pipeline (always returns GroupedNodes)
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

        // 5. Check cancellation after pipeline
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

        // 6. Send result
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

    fn page_from_cached_listing(
        entries: Vec<crate::FileNode>,
        request: &DirectoryPageRequest,
    ) -> Result<DirectoryPageResult, CoreError> {
        validate_page_limit(request.limit)?;
        let start = parse_offset_cursor(request.cursor.as_ref())?;
        let end = start.saturating_add(request.limit).min(entries.len());
        let page_entries = if start < entries.len() {
            entries[start..end].to_vec()
        } else {
            Vec::new()
        };
        let state = if end < entries.len() {
            DirectoryPageState::partial(
                page_entries.len(),
                Some(entries.len()),
                DirectoryCursor(end.to_string()),
            )
        } else {
            DirectoryPageState::complete(page_entries.len(), Some(entries.len()))
        };
        Ok(DirectoryPageResult {
            entries: page_entries,
            state,
        })
    }

    fn filtered_page_from_cached_listing(
        entries: Vec<crate::FileNode>,
        request: &DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
    ) -> Result<DirectoryPageResult, CoreError> {
        validate_page_limit(request.limit)?;
        if request
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.0.starts_with(FILTER_CURSOR_PREFIX))
        {
            return Err(CoreError::invalid_input(
                "Filtered provider cursors cannot be used with cached listings",
            ));
        }

        let pipeline = Pipeline::from_config(pipeline_config);
        let entries = pipeline.execute_flat(entries);
        Self::page_from_cached_listing(entries, request)
    }

    async fn load_filtered_page(
        provider: &dyn FsProvider,
        path: &Path,
        request: DirectoryPageRequest,
        pipeline_config: &PipelineConfig,
        cancel: &CancellationToken,
    ) -> Result<FilteredPageLoad, CoreError> {
        validate_page_limit(request.limit)?;
        let mut provider_cursor =
            Self::decode_filter_cursor(request.cursor.as_ref(), request.listing, pipeline_config)?;
        let raw_budget = request
            .limit
            .saturating_mul(4)
            .max(crate::DEFAULT_DIRECTORY_PAGE_SIZE);
        let mut raw_read = 0usize;
        let mut filtered_entries = Vec::with_capacity(request.limit);
        let pipeline = Pipeline::from_config(pipeline_config);

        loop {
            if cancel.is_cancelled() {
                return Ok(FilteredPageLoad::Cancelled);
            }

            if filtered_entries.len() >= request.limit {
                return Ok(FilteredPageLoad::Page(Self::filtered_page_result(
                    filtered_entries,
                    provider_cursor,
                    false,
                    request.listing,
                    pipeline_config,
                )?));
            }

            let remaining_budget = raw_budget.saturating_sub(raw_read);
            if remaining_budget == 0 {
                return Ok(FilteredPageLoad::Page(Self::filtered_page_result(
                    filtered_entries,
                    provider_cursor,
                    false,
                    request.listing,
                    pipeline_config,
                )?));
            }

            let remaining_output = request.limit - filtered_entries.len();
            let raw_limit = remaining_budget.min(remaining_output).max(1);
            let raw_page = provider
                .list_page(
                    path,
                    DirectoryPageRequest {
                        listing: request.listing,
                        limit: raw_limit,
                        cursor: provider_cursor.clone(),
                    },
                )
                .await?;

            if cancel.is_cancelled() {
                return Ok(FilteredPageLoad::Cancelled);
            }

            raw_read = raw_read.saturating_add(raw_page.state.page_count);
            let raw_complete = raw_page.state.complete;
            provider_cursor = raw_page.state.next_cursor.clone();
            filtered_entries.extend(pipeline.execute_flat(raw_page.entries));

            if raw_complete {
                return Ok(FilteredPageLoad::Page(Self::filtered_page_result(
                    filtered_entries,
                    None,
                    true,
                    request.listing,
                    pipeline_config,
                )?));
            }

            if raw_read >= raw_budget || raw_page.state.page_count == 0 {
                return Ok(FilteredPageLoad::Page(Self::filtered_page_result(
                    filtered_entries,
                    provider_cursor,
                    false,
                    request.listing,
                    pipeline_config,
                )?));
            }
        }
    }

    fn filtered_page_result(
        entries: Vec<crate::FileNode>,
        provider_cursor: Option<DirectoryCursor>,
        complete: bool,
        listing: ListingOptions,
        pipeline_config: &PipelineConfig,
    ) -> Result<DirectoryPageResult, CoreError> {
        let state = if complete || provider_cursor.is_none() {
            DirectoryPageState::complete(entries.len(), None)
        } else {
            DirectoryPageState::partial(
                entries.len(),
                None,
                Self::encode_filter_cursor(provider_cursor, listing, pipeline_config)?,
            )
        };
        Ok(DirectoryPageResult { entries, state })
    }

    fn encode_filter_cursor(
        provider_cursor: Option<DirectoryCursor>,
        listing: ListingOptions,
        pipeline_config: &PipelineConfig,
    ) -> Result<DirectoryCursor, CoreError> {
        let listing_json = serde_json::to_string(&listing)
            .map_err(|e| CoreError::invalid_input(format!("Invalid listing cursor: {e}")))?;
        let pipeline_json = serde_json::to_string(pipeline_config)
            .map_err(|e| CoreError::invalid_input(format!("Invalid pipeline cursor: {e}")))?;
        let provider_cursor = provider_cursor.map(|cursor| cursor.0).unwrap_or_default();
        Ok(DirectoryCursor(format!(
            "{FILTER_CURSOR_PREFIX}{}:{}:{}:{}{}{}",
            listing_json.len(),
            pipeline_json.len(),
            provider_cursor.len(),
            listing_json,
            pipeline_json,
            provider_cursor
        )))
    }

    fn decode_filter_cursor(
        cursor: Option<&DirectoryCursor>,
        listing: ListingOptions,
        pipeline_config: &PipelineConfig,
    ) -> Result<Option<DirectoryCursor>, CoreError> {
        let Some(cursor) = cursor else {
            return Ok(None);
        };
        let Some(payload) = cursor.0.strip_prefix(FILTER_CURSOR_PREFIX) else {
            return Err(CoreError::invalid_input(
                "Filtered page requests require a filtered cursor",
            ));
        };

        let parts: Vec<&str> = payload.splitn(4, ':').collect();
        if parts.len() != 4 {
            return Err(CoreError::invalid_input("Invalid filtered cursor"));
        }
        let listing_len = parts[0]
            .parse::<usize>()
            .map_err(|_| CoreError::invalid_input("Invalid filtered cursor listing length"))?;
        let pipeline_len = parts[1]
            .parse::<usize>()
            .map_err(|_| CoreError::invalid_input("Invalid filtered cursor pipeline length"))?;
        let provider_len = parts[2]
            .parse::<usize>()
            .map_err(|_| CoreError::invalid_input("Invalid filtered cursor provider length"))?;
        let data = parts[3];
        if data.len() != listing_len + pipeline_len + provider_len {
            return Err(CoreError::invalid_input("Invalid filtered cursor payload"));
        }

        let listing_json = data[..listing_len].to_string();
        let pipeline_json = data[listing_len..listing_len + pipeline_len].to_string();
        let provider_cursor = data[listing_len + pipeline_len..].to_string();
        let expected_listing_json = serde_json::to_string(&listing)
            .map_err(|e| CoreError::invalid_input(format!("Invalid listing cursor: {e}")))?;
        let expected_pipeline_json = serde_json::to_string(pipeline_config)
            .map_err(|e| CoreError::invalid_input(format!("Invalid pipeline cursor: {e}")))?;
        if listing_json != expected_listing_json || pipeline_json != expected_pipeline_json {
            return Err(CoreError::invalid_input(
                "Filtered cursor does not match requested listing or pipeline",
            ));
        }

        Ok((!provider_cursor.is_empty()).then_some(DirectoryCursor(provider_cursor)))
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

        let groups = Pipeline::default().execute_grouped(page.entries);
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
                Event::DirectoryEntryPageLoaded {
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
                Event::DirectoryPageLoaded {
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
