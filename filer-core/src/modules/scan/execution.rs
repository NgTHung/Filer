//! # Scan execution inputs
//!
//! These inputs keep a scan's target, event correlation, and borrowed runtime
//! resources together as work moves from dispatch to result emission.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//! let parent = LocationRef::from_location(&Location::local("/tmp"));
//! assert!(parent.descriptor().is_some());
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rapidhash::fast::RandomState;

use crate::actors::cancel::CancellationToken;
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::directory::{DirectoryLoadState, DirectoryPageResult};
use crate::model::location::{LocationId, LocationRef};
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget, ProgressUnit,
};
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::pipeline::{GroupedEntries, Pipeline, PipelineConfig};
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::vfs::provider::FsProvider;

use super::paging::PagingSessions;

mod cache;
mod full;
mod paged;
mod segmented;

pub(super) use cache::{CacheScan, invalidate_cache, scan_cached};
pub(super) use full::{FullScan, scan_full};
pub(super) use paged::{PagedScan, scan_page};
pub(super) use segmented::scan_segmented_location;

pub(super) struct ScanTarget {
    pub path: PathBuf,
    pub parent_location: LocationRef,
    pub parent_location_id: Option<LocationId>,
}

#[derive(Clone, Copy)]
pub(super) struct ScanEvents<'a> {
    pub events: &'a EventSink,
    pub latest_scans: &'a scc::HashMap<SessionId, RequestId, RandomState>,
    pub session: SessionId,
    pub request: RequestId,
}

pub(super) struct ScanResources<'a> {
    pub provider: &'a Arc<dyn FsProvider>,
    pub cancel: &'a CancellationToken,
    pub cache: Option<&'a SharedDirCache>,
    pub paging: &'a PagingSessions,
}

pub(super) async fn emit_page_result(
    scan_events: ScanEvents<'_>,
    path: &Path,
    parent_location: LocationRef,
    page: DirectoryPageResult,
    pipeline_config: &PipelineConfig,
    context: &'static str,
) {
    let ScanEvents {
        events,
        latest_scans,
        session,
        request,
    } = scan_events;
    let page_state = page.state.clone();
    emit_scan_progress(
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
            scan_target(path, Some(&parent_location)),
        ),
    )
    .await;
    if !is_latest(latest_scans, session, request) {
        return;
    }

    let groups = Pipeline::from_config(pipeline_config).execute_grouped(page.entries);
    emit_scan_progress(
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
            scan_target(path, Some(&parent_location)),
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
    emit_scan_progress(
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
            scan_target(path, Some(&parent_location)),
        ),
    )
    .await;
}

pub(super) async fn emit_scan_progress(
    events: &EventSink,
    latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
    session: SessionId,
    request: RequestId,
    snapshot: ProgressSnapshot,
) {
    if !is_latest(latest_scans, session, request) {
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

pub(super) fn scan_target(path: &Path, location: Option<&LocationRef>) -> Option<ProgressTarget> {
    location
        .cloned()
        .map(ProgressTarget::Location)
        .or_else(|| Some(ProgressTarget::Path(path.to_path_buf())))
}

pub(super) fn is_latest(
    latest_scans: &scc::HashMap<SessionId, RequestId, RandomState>,
    session: SessionId,
    request: RequestId,
) -> bool {
    latest_scans
        .read_sync(&session, |_, latest| *latest == request)
        .unwrap_or(false)
}

pub(super) fn limited_entries(
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
