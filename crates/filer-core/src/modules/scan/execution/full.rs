//! # Full scan execution
//!
//! This module lists and transforms a complete directory snapshot while the
//! scanner actor owns request dispatch and work lifetime.
//!
//! ```
//! use filer_core::DirectoryLoadOptions;
//! let load = DirectoryLoadOptions::unbounded(Default::default());
//! assert!(!load.is_bounded());
//! ```

use std::path::Path;

use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::errors::ErrorCode;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{LocationId, LocationRef};
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;

use super::cache::store_snapshot;
use super::{ScanEvents, emit_scan_progress, is_latest, limited_entries, scan_target};

pub(in crate::modules::scan) struct FullScan<'a> {
    pub provider: &'a dyn FsProvider,
    pub cancel: &'a CancellationToken,
    pub cache: Option<&'a SharedDirCache>,
    pub events: ScanEvents<'a>,
    pub path: &'a Path,
    pub parent_location: &'a LocationRef,
    pub parent_location_id: Option<LocationId>,
    pub pipeline_config: &'a PipelineConfig,
    pub load_options: &'a DirectoryLoadOptions,
}

pub(in crate::modules::scan) async fn scan_full(scan: FullScan<'_>) {
    let FullScan {
        provider,
        cancel,
        cache,
        events: scan_events,
        path,
        parent_location,
        parent_location_id,
        pipeline_config,
        load_options,
    } = scan;
    let ScanEvents {
        events,
        latest_scans,
        session,
        request,
    } = scan_events;
    let cx = ProviderCx::with_cancel(cancel);
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
                    scan_target(path, Some(parent_location)),
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
                        scan_target(path, Some(parent_location)),
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

    if !load_options.is_bounded() {
        store_snapshot(
            cache,
            parent_location,
            parent_location_id,
            path,
            load_options.listing,
            &entries,
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
                scan_target(path, Some(parent_location)),
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
            scan_target(path, Some(parent_location)),
        ),
    )
    .await;
    let groups = Pipeline::from_config(pipeline_config).execute_grouped(entries);
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
            scan_target(path, Some(parent_location)),
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
                scan_target(path, Some(parent_location)),
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
            scan_target(path, Some(parent_location)),
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
            scan_target(path, Some(parent_location)),
        ),
    )
    .await;
}
