use crate::OperationId;
use crate::modules::operations::queue::{AdmissionError, MutationQueue};

#[test]
fn running_work_reserves_capacity_until_it_settles() {
    let mut queue = MutationQueue::<&str>::default();
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
    let mut queue = MutationQueue::<&str>::default();
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

#[test]
fn pause_holds_the_recovery_slot_and_preserves_waiting_order() {
    let mut queue = MutationQueue::<&str>::default();
    let failed = OperationId(1);
    let waiting = OperationId(2);
    let another = OperationId(3);

    assert_eq!(queue.admit(failed, "copy", 2), Ok(()));
    let (_, work, _) = queue.start().unwrap();
    assert_eq!(queue.admit(waiting, "delete", 2), Ok(()));
    queue.finish();
    queue.pause(failed, work);

    assert_eq!(queue.paused_id(), Some(failed));
    assert_eq!(queue.waiting().copied().collect::<Vec<_>>(), vec!["delete"]);
    assert_eq!(queue.paused_work_mut().map(|work| *work), Some("copy"));
    assert!(queue.start().is_none());
    assert_eq!(queue.admit(another, "create", 2), Err(AdmissionError::Full));
    assert!(!queue.continue_after(another));
    assert!(queue.continue_after(failed));
    assert_eq!(queue.admit(another, "create", 2), Ok(()));
    assert!(matches!(queue.start(), Some((id, "delete", _)) if id == waiting));
}

#[test]
fn retry_uses_the_reserved_slot_and_can_pause_again() {
    let mut queue = MutationQueue::<&str>::default();
    let failed = OperationId(1);
    let waiting = OperationId(2);
    let retry = OperationId(3);

    assert_eq!(queue.admit(waiting, "delete", 2), Ok(()));
    queue.pause(failed, "copy");
    assert!(!queue.retry(failed, waiting, "duplicate"));
    assert!(!queue.retry(failed, failed, "same attempt"));
    assert!(queue.retry(failed, retry, "retry copy"));
    let (id, work, _) = queue.start().unwrap();
    assert_eq!((id, work), (retry, "retry copy"));

    queue.finish();
    queue.pause(retry, work);
    assert!(!queue.continue_after(failed));
    assert!(queue.start().is_none());
    assert!(queue.continue_after(retry));
    assert_eq!(queue.drain_waiting().collect::<Vec<_>>(), vec!["delete"]);
    assert!(queue.is_empty());
}
