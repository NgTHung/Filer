//! # Operation support
//!
//! This module centralizes provider context, event construction, and cache
//! invalidation so every operation handler applies the same lifecycle rules.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//!
//! let location = Location::local("/tmp/example");
//! let affected = LocationRef::from_location(&location);
//! assert_eq!(affected.id(), Some(location.id()));
//! ```

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use rapidhash::fast::RandomState;

use crate::actors::cancel::CancellationToken;
use crate::api::event_sink::EventSink;
use crate::api::events::Event;
use crate::model::location::LocationRef;
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{ProgressScope, ProgressSnapshot};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::services::dir_cache::SharedDirCache;
use crate::utils::channel::send_or_warn_async;
use crate::{CoreError, ErrorCode, ProviderCx};

use super::command::OperationEventMode;

pub(crate) type TrashFn = Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>;

pub(super) fn operation_cx(
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> ProviderCx<'_> {
    let cx = ProviderCx::with_cancel(cancel);
    match deadline {
        Some(deadline) => cx.with_deadline(deadline),
        None => cx,
    }
}

pub(super) fn operation_error(
    err: CoreError,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
) -> Event {
    Event::from_operation_error(err, session, request, operation)
}

pub(super) fn operation_complete_event(
    _registry: &NodeRegistry,
    kind: OperationKind,
    operation: OperationId,
    affected: Vec<LocationRef>,
    session: SessionId,
    _event_mode: OperationEventMode,
) -> Result<Event, CoreError> {
    Ok(Event::OperationComplete {
        operation_id: operation,
        operation: kind,
        success: true,
        affected,
        session,
    })
}

pub(super) async fn emit_operation_progress(
    events: &EventSink,
    kind: OperationKind,
    session: SessionId,
    request: RequestId,
    operation: OperationId,
    snapshot: ProgressSnapshot,
) {
    send_or_warn_async(
        events,
        Event::ProgressUpdated {
            scope: ProgressScope::operation(kind, session, request, operation),
            snapshot,
        },
        "operator: progress",
    )
    .await;
}

pub(super) fn is_cross_device(err: &CoreError) -> bool {
    err.code() == ErrorCode::IoFailed
        && (err.message.contains("cross-device")
            || err.message.contains("os error 18")
            || err.message.contains("os error 17"))
}

pub(super) fn invalidate_parent_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let (Some(parent), Some(c)) = (path.parent(), cache)
        && let Ok(mut guard) = c.lock()
    {
        guard.invalidate(crate::Location::local(parent.to_path_buf()).id());
    }
}

pub(super) fn invalidate_subtree_cache(cache: &Option<SharedDirCache>, path: &Path) {
    if let Some(c) = cache
        && let Ok(mut guard) = c.lock()
    {
        guard.invalidate_local_subtree(path);
    }
}

pub(super) async fn remove_operation_if_current(
    active_operation_ids: Arc<scc::HashMap<SessionId, OperationId, RandomState>>,
    session: SessionId,
    operation: OperationId,
) {
    let _ = active_operation_ids
        .remove_if_async(&session, |current| *current == operation)
        .await;
}
