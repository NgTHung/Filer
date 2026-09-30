//! # Scan cache execution
//!
//! This module owns directory cache lookup, invalidation, and storage so each
//! execution path applies the same cache identity rules.
//!
//! ```
//! use filer_core::{DirectoryLoadOptions, ListingOptions};
//! let load = DirectoryLoadOptions::unbounded(ListingOptions::fast());
//! assert!(!load.is_paged());
//! ```

use std::path::Path;

use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationId, LocationRef};
use crate::model::node::NodeEntry;
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::pipeline::{Pipeline, PipelineConfig, effective_listing};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::ListingOptions;

use super::{ScanEvents, emit_page_result, emit_scan_progress, is_latest, scan_target};
use crate::modules::scan::paging::{PageLoad, PagingSessions};

pub(in crate::modules::scan) struct CacheScan<'a> {
    pub cache: Option<&'a SharedDirCache>,
    pub paging: &'a PagingSessions,
    pub cancel: &'a CancellationToken,
    pub events: ScanEvents<'a>,
    pub path: &'a Path,
    pub parent_location: &'a LocationRef,
    pub parent_location_id: Option<LocationId>,
    pub pipeline_config: &'a PipelineConfig,
    pub load_options: &'a DirectoryLoadOptions,
}

pub(in crate::modules::scan) fn invalidate_cache(scan: &CacheScan<'_>) {
    let ScanEvents { session, .. } = scan.events;
    scan.paging.clear_session(session);
    if let Some(cache) = scan.cache
        && let Ok(mut cache) = cache.lock()
    {
        tracing::debug!(path = %scan.path.display(), "Invalidating directory cache before scan");
        if let Some(location_id) = scan.parent_location_id {
            cache.invalidate(location_id);
        } else {
            cache.invalidate_local_subtree(scan.path);
        }
    }
}

pub(in crate::modules::scan) async fn scan_cached(scan: &CacheScan<'_>) -> bool {
    let ScanEvents {
        events,
        latest_scans,
        session,
        request,
    } = scan.events;
    let cache_listing = if scan.load_options.is_paged() {
        effective_listing(scan.pipeline_config, scan.load_options.listing)
    } else {
        scan.load_options.listing
    };
    let cached = scan.cache.and_then(|cache| {
        let mut cache = cache.lock().ok()?;
        let location_id = scan.parent_location_id?;
        cache.get(location_id, cache_listing)
    });
    let Some(cached) = cached else {
        return false;
    };

    tracing::trace!(path = %scan.path.display(), session = %session, "Directory scan served from cache");
    if let Some(page_request) = scan.load_options.page_request() {
        let cx = ProviderCx::with_cancel(scan.cancel);
        match scan.paging.load_cached(
            cached,
            scan.path,
            session,
            page_request,
            scan.pipeline_config,
            &cx,
        ) {
            Ok(PageLoad::Page(page)) => {
                emit_page_result(
                    scan.events,
                    scan.path,
                    scan.parent_location.clone(),
                    page,
                    scan.pipeline_config,
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
                        scan_target(scan.path, Some(scan.parent_location)),
                    ),
                )
                .await;
            }
            Err(error) => {
                if is_latest(latest_scans, session, request) {
                    send_or_warn_async(
                        events,
                        Event::from_request_error(error, session, request),
                        "scan cached page error",
                    )
                    .await;
                }
            }
        }
        return true;
    }

    let pipeline = Pipeline::from_config(scan.pipeline_config);
    let (groups, load) = pipeline
        .execute_grouped(cached)
        .limited(scan.load_options.snapshot_limit());
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
            scan_target(scan.path, Some(scan.parent_location)),
        ),
    )
    .await;
    if !is_latest(latest_scans, session, request) {
        return true;
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
            scan_target(scan.path, Some(scan.parent_location)),
        ),
    )
    .await;
    send_or_warn_async(
        events,
        Event::DirectoryLoaded {
            parent: scan.parent_location.clone(),
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
            scan_target(scan.path, Some(scan.parent_location)),
        ),
    )
    .await;
    true
}

pub(in crate::modules::scan) fn store_snapshot(
    cache: Option<&SharedDirCache>,
    parent: &LocationRef,
    parent_id: Option<LocationId>,
    path: &Path,
    listing: ListingOptions,
    entries: &[NodeEntry],
) {
    if let Some(cache) = cache
        && let Ok(mut cache) = cache.lock()
        && parent_id.is_some()
    {
        cache.put(cache_location(parent, path), listing, entries.to_vec());
    }
}

fn cache_location(parent: &LocationRef, path: &Path) -> Location {
    parent
        .descriptor()
        .cloned()
        .map(Location::new)
        .unwrap_or_else(|| Location::local(path.to_path_buf()))
}
