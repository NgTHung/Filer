//! # Segmented scan execution
//!
//! This module resolves and lists nested provider locations while the scanner
//! actor retains request lifetime and cancellation ownership.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//! let location = Location::local("/tmp");
//! assert_eq!(LocationRef::from_location(&location).id(), Some(location.id()));
//! ```

use std::sync::Arc;

use crate::actors::cancel::CancellationToken;
use crate::api::events::Event;
use crate::errors::ErrorCode;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{LocationDescriptor, LocationRef};
use crate::model::progress::{ProgressPhase, ProgressSnapshot, ProgressStatus, ProgressUnit};
use crate::pipeline::{Pipeline, PipelineConfig};
use crate::utils::channel::send_or_warn_async;
use crate::vfs::context::ProviderCx;
use crate::vfs::provider::FsProvider;
use crate::vfs::segmented::SegmentedLocationResolver;

use super::{ScanEvents, emit_scan_progress, is_latest, limited_entries, scan_target};

pub(in crate::modules::scan) async fn scan_segmented_location(
    provider: &Arc<dyn FsProvider>,
    scan_events: ScanEvents<'_>,
    descriptor: LocationDescriptor,
    parent: LocationRef,
    pipeline_config: PipelineConfig,
    load_options: DirectoryLoadOptions,
    cancel: &CancellationToken,
) {
    let ScanEvents {
        events,
        latest_scans,
        session,
        request,
    } = scan_events;
    let target_path = descriptor.display_path();
    let target = std::path::Path::new(&target_path);
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
            scan_target(target, Some(&parent)),
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
                    scan_target(target, Some(&parent)),
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
                        scan_target(target, Some(&parent)),
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

    if cancel.is_cancelled() || !is_latest(latest_scans, session, request) {
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
            scan_target(target, None),
        ),
    )
    .await;
}
