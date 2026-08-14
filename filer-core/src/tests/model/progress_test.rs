use std::path::PathBuf;

use crate::model::location::{Location, LocationRef};
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{
    ProgressKind, ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressTarget,
    ProgressUnit,
};
use crate::model::request::RequestId;
use crate::model::session::SessionId;

#[test]
fn test_scan_progress_scope_sets_request_without_operation() {
    let session = SessionId::new();
    let request = RequestId::new();

    let scope = ProgressScope::scan(session, request);

    assert_eq!(scope.kind, ProgressKind::Scan);
    assert_eq!(scope.session, session);
    assert_eq!(scope.request, Some(request));
    assert_eq!(scope.operation, None);
}

#[test]
fn test_operation_progress_scope_sets_request_and_operation() {
    let session = SessionId::new();
    let request = RequestId::new();
    let operation = OperationId::new();

    let scope = ProgressScope::operation(OperationKind::Delete, session, request, operation);

    assert_eq!(scope.kind, ProgressKind::Operation(OperationKind::Delete));
    assert_eq!(scope.session, session);
    assert_eq!(scope.request, Some(request));
    assert_eq!(scope.operation, Some(operation));
}

#[test]
fn test_progress_snapshot_supports_unknown_total_and_target() {
    let snapshot = ProgressSnapshot::new(
        ProgressStatus::Running,
        ProgressPhase::Processing,
        ProgressUnit::Item,
        3,
        None,
        Some(ProgressTarget::Path(PathBuf::from("/tmp/progress"))),
    );

    assert_eq!(snapshot.done, 3);
    assert_eq!(snapshot.total, None);
    assert!(matches!(snapshot.current, Some(ProgressTarget::Path(_))));
}

#[test]
fn test_progress_snapshot_supports_location_target() {
    let location = LocationRef::from_location(&Location::local("/tmp/progress-location"));
    let snapshot = ProgressSnapshot::new(
        ProgressStatus::Running,
        ProgressPhase::Processing,
        ProgressUnit::Item,
        1,
        Some(2),
        Some(ProgressTarget::Location(location.clone())),
    );

    assert_eq!(snapshot.current, Some(ProgressTarget::Location(location)));
}
