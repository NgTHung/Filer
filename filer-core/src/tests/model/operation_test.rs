use crate::model::operation::OperationId;

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
