use crate::OperationId;
use crate::modules::operations::queue::{AdmissionError, MutationQueue};

#[test]
fn running_work_reserves_capacity_until_it_settles() {
    let mut queue = MutationQueue::default();
    let first = OperationId(1);
    let second = OperationId(2);
    let third = OperationId(3);

    assert_eq!(queue.admit(first, "copy", 2), Ok(()));
    let (id, work, token) = queue.start().unwrap();
    assert_eq!((id, work), (first, "copy"));
    assert_eq!(queue.admit(second, "create", 2), Ok(()));
    assert_eq!(
        queue.admit(first, "duplicate", 2),
        Err(AdmissionError::Duplicate)
    );
    assert_eq!(queue.admit(third, "delete", 2), Err(AdmissionError::Full));
    assert!(queue.start().is_none());
    assert!(!token.is_cancelled());

    assert!(queue.cancel_running(first));
    assert!(token.is_cancelled());
    assert_eq!(queue.admit(third, "delete", 2), Err(AdmissionError::Full));

    queue.finish();
    assert_eq!(queue.admit(third, "delete", 2), Ok(()));
    assert!(matches!(queue.start(), Some((id, "create", _)) if id == second));
    queue.finish();
    assert!(matches!(queue.start(), Some((id, "delete", _)) if id == third));
    queue.finish();
    assert!(queue.is_empty());
}

#[test]
fn cancelling_waiting_work_frees_only_its_slot() {
    let mut queue = MutationQueue::default();
    let first = OperationId(1);
    let second = OperationId(2);
    let third = OperationId(3);

    assert_eq!(queue.admit(first, "copy", 2), Ok(()));
    let (_, _, token) = queue.start().unwrap();
    assert_eq!(queue.admit(second, "create", 2), Ok(()));
    assert_eq!(queue.remove(second), Some("create"));
    assert_eq!(queue.remove(second), None);
    assert!(!queue.cancel_running(second));
    assert!(!token.is_cancelled());
    assert_eq!(queue.admit(third, "delete", 2), Ok(()));
    queue.finish();
    assert!(matches!(queue.start(), Some((id, "delete", _)) if id == third));
}
