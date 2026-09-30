//! Tests for `ProviderCx` deadline and cancellation racing.

use std::time::Duration;

use crate::errors::{CoreError, ErrorCode, ErrorContext, ErrorKind, ErrorTarget};
use crate::model::cancel::CancelSignal;
use crate::vfs::context::ProviderCx;

async fn ok_after(delay: Duration) -> Result<u32, CoreError> {
    tokio::time::sleep(delay).await;
    Ok(7)
}

#[tokio::test]
async fn test_race_returns_value_without_deadline_or_cancel() {
    let cx = ProviderCx::none();
    let value = cx.race("mock", ok_after(Duration::ZERO)).await.unwrap();
    assert_eq!(value, 7);
}

#[tokio::test]
async fn test_race_times_out_with_provider_context() {
    let cx = ProviderCx::none().with_timeout(Duration::from_millis(20));
    let err = cx
        .race("mock", ok_after(Duration::from_secs(30)))
        .await
        .expect_err("deadline should fire before the slow future");

    assert_eq!(err.code(), ErrorCode::TimedOut);
    assert_eq!(err.kind(), ErrorKind::Timeout);
    assert_eq!(
        err.target(),
        Some(&ErrorTarget::Provider("mock".to_string()))
    );
    assert!(matches!(
        err.context(),
        Some(ErrorContext::Timeout { provider }) if provider == "mock"
    ));
}

#[tokio::test]
async fn test_race_cancels_in_flight_future() {
    let signal = CancelSignal::new();
    let cx = ProviderCx::with_cancel(&signal);
    let canceller = signal.clone();

    let work = cx.race("mock", ok_after(Duration::from_secs(30)));
    let trigger = async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        canceller.cancel();
    };

    let (result, ()) = tokio::join!(work, trigger);
    let err = result.expect_err("cancel should interrupt the slow future");
    assert_eq!(err.code(), ErrorCode::Cancelled);
}

#[tokio::test]
async fn test_race_returns_cancelled_when_already_cancelled() {
    let signal = CancelSignal::new();
    signal.cancel();
    let cx = ProviderCx::with_cancel(&signal);

    let err = cx
        .race("mock", ok_after(Duration::ZERO))
        .await
        .expect_err("already-cancelled context should short-circuit");
    assert_eq!(err.code(), ErrorCode::Cancelled);
}
