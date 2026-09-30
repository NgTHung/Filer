use serde_json::json;

use crate::model::location::{Location, LocationRef};
use crate::model::operation::{
    OperationConflictPolicy, OperationConflictResolution, OperationId, OperationKind,
    OperationProviderGuarantee, OperationUndoMode, OperationUndoRecord,
};

#[test]
fn new_operation_ids_are_monotonic() {
    let first = OperationId::new();
    let second = OperationId::new();

    assert!(second.0 > first.0);
}

#[test]
fn default_operation_id_is_sentinel_zero() {
    assert_eq!(OperationId::default(), OperationId::DEFAULT);
    assert_eq!(OperationId::DEFAULT.0, 0);
    assert_eq!(OperationId::DEFAULT.to_string(), "operation:0");
}

#[test]
fn conflict_resolutions_use_stable_snake_case_labels() {
    assert_eq!(
        serde_json::to_value(OperationConflictResolution::RenameIncoming).unwrap(),
        json!("rename_incoming")
    );
    assert_eq!(
        serde_json::from_value::<OperationConflictResolution>(json!("merge_directory")).unwrap(),
        OperationConflictResolution::MergeDirectory
    );
}

#[test]
fn unknown_conflict_resolution_round_trips_for_forward_compatibility() {
    let decoded: OperationConflictResolution =
        serde_json::from_value(json!("provider_prompt")).unwrap();

    assert_eq!(
        decoded,
        OperationConflictResolution::Unknown("provider_prompt".to_string())
    );
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        json!("provider_prompt")
    );
}

#[test]
fn default_conflict_policy_fails_for_file_and_directory_conflicts() {
    let policy = OperationConflictPolicy::default();

    assert_eq!(policy.default, OperationConflictResolution::Fail);
    assert_eq!(policy.file, None);
    assert_eq!(policy.directory, None);
    assert_eq!(policy.file_resolution(), OperationConflictResolution::Fail);
    assert_eq!(
        policy.directory_resolution(),
        OperationConflictResolution::Fail
    );
}

#[test]
fn conflict_policy_can_describe_copy_and_move_overrides() {
    let policy = OperationConflictPolicy {
        default: OperationConflictResolution::Skip,
        file: Some(OperationConflictResolution::Replace),
        directory: Some(OperationConflictResolution::MergeDirectory),
    };

    let value = serde_json::to_value(&policy).unwrap();
    assert_eq!(value["default"], json!("skip"));
    assert_eq!(value["file"], json!("replace"));
    assert_eq!(value["directory"], json!("merge_directory"));
    assert_eq!(
        serde_json::from_value::<OperationConflictPolicy>(value).unwrap(),
        policy
    );
}

#[test]
fn provider_guarantees_cover_atomic_best_effort_and_unknown_values() {
    assert_eq!(
        serde_json::to_value(OperationProviderGuarantee::Atomic).unwrap(),
        json!("atomic")
    );
    assert_eq!(
        serde_json::from_value::<OperationProviderGuarantee>(json!("best_effort")).unwrap(),
        OperationProviderGuarantee::BestEffort
    );
    assert_eq!(
        serde_json::from_value::<OperationProviderGuarantee>(json!("transactional")).unwrap(),
        OperationProviderGuarantee::Unknown("transactional".to_string())
    );
}

#[test]
fn undo_record_serializes_reversal_metadata() {
    let source = LocationRef::from_location(&Location::local("/from/report.txt"));
    let destination = LocationRef::from_location(&Location::local("/to/report.txt"));
    let record = OperationUndoRecord {
        operation: OperationKind::Move,
        sources: vec![source.clone()],
        destinations: vec![destination.clone()],
        affected: vec![destination],
        trash: false,
        undo: OperationUndoMode::BestEffort,
        guarantee: OperationProviderGuarantee::BestEffort,
    };

    let value = serde_json::to_value(&record).unwrap();
    assert_eq!(value["operation"], json!("move"));
    assert_eq!(value["trash"], json!(false));
    assert_eq!(value["undo"], json!("best_effort"));
    assert_eq!(value["guarantee"], json!("best_effort"));

    let decoded: OperationUndoRecord = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.operation, OperationKind::Move);
    assert_eq!(decoded.sources, vec![source]);
    assert_eq!(decoded.undo, OperationUndoMode::BestEffort);
}
