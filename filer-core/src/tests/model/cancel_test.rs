//! Tests for the `CancelSignal` primitive.

use std::time::Duration;

use crate::model::cancel::CancelSignal;

#[test]
fn test_new_signal_is_not_cancelled() {
    let signal = CancelSignal::new();
    assert!(!signal.is_cancelled());
}

#[test]
fn test_cancel_sets_flag_for_all_clones() {
    let signal = CancelSignal::new();
    let clone = signal.clone();
    signal.cancel();
    assert!(signal.is_cancelled());
    assert!(clone.is_cancelled());
}

#[tokio::test]
async fn test_cancelled_resolves_after_cancel() {
    let signal = CancelSignal::new();
    let waiter = signal.clone();
    let task = tokio::spawn(async move { waiter.cancelled().await });

    signal.cancel();

    let joined = tokio::time::timeout(Duration::from_secs(1), task).await;
    assert!(joined.is_ok(), "cancelled() should resolve once cancel fires");
    joined.unwrap().unwrap();
}

#[tokio::test]
async fn test_cancelled_returns_immediately_when_already_cancelled() {
    let signal = CancelSignal::new();
    signal.cancel();

    let joined = tokio::time::timeout(Duration::from_secs(1), signal.cancelled()).await;
    assert!(joined.is_ok(), "cancelled() must not block when already cancelled");
}

#[tokio::test]
async fn test_cancelled_pending_until_cancel() {
    let signal = CancelSignal::new();
    let joined = tokio::time::timeout(Duration::from_millis(50), signal.cancelled()).await;
    assert!(joined.is_err(), "cancelled() should block until cancel fires");
}
