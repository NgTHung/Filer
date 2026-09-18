//! # Paged scan execution
//!
//! This module loads provider-backed directory pages and preserves paging,
//! cancellation, cache, and result emission semantics behind one interface.
//!
//! ```
//! use filer_core::DirectoryLoadOptions;
//! let load = DirectoryLoadOptions::page(64);
//! assert!(load.is_paged());
//! ```

use std::path::Path;

use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::model::directory::DirectoryPageRequest;
use crate::model::location::{LocationId, LocationRef};
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::pipeline::PipelineConfig;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::{FsProvider, ListingOptions};

use super::cache::store_snapshot;
use super::{ScanEvents, emit_page_result, emit_scan_progress, is_latest, scan_target};
use crate::modules::scan::paging::{PageLoad, PagingSessions};

pub(in crate::modules::scan) struct PagedScan<'a> {
    pub provider: &'a dyn FsProvider,
    pub cache: Option<&'a SharedDirCache>,
    pub paging: &'a PagingSessions,
    pub cancel: &'a CancellationToken,
    pub events: ScanEvents<'a>,
    pub path: &'a Path,
    pub parent_location: &'a LocationRef,
    pub parent_location_id: Option<LocationId>,
    pub pipeline_config: &'a PipelineConfig,
    pub listing: ListingOptions,
    pub page_request: DirectoryPageRequest,
}

pub(in crate::modules::scan) async fn scan_page(scan: PagedScan<'_>) {
    let PagedScan {
        provider,
        cache,
        paging,
        cancel,
        events: scan_events,
        path,
        parent_location,
        parent_location_id,
        pipeline_config,
        listing,
        page_request,
    } = scan;
    let ScanEvents {
        events,
        latest_scans,
        session,
        request,
    } = scan_events;
    let first_page = page_request.cursor.is_none();
    let cx = ProviderCx::with_cancel(cancel);
    let page = match paging
        .load_provider(provider, path, session, page_request, pipeline_config, &cx)
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
                    scan_target(path, Some(parent_location)),
                ),
            )
            .await;
            return;
        }
        Err(error) => {
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
                        scan_target(path, Some(parent_location)),
                    ),
                )
                .await;
                send_or_warn_async(
                    events,
                    Event::from_request_error(error, session, request),
                    "scan page error",
                )
                .await;
            }
            return;
        }
    };

    if first_page && page.state.complete && pipeline_config == &PipelineConfig::default() {
        store_snapshot(
            cache,
            parent_location,
            parent_location_id,
            path,
            listing,
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
                scan_target(path, Some(parent_location)),
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
        pipeline_config,
        "scan page result",
    )
    .await;
}
