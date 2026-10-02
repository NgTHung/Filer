use std::time::Duration;

use crate::actors::WorkTracker;
use crate::api::event_sink::{DEFAULT_EVENT_CHANNEL_CAPACITY, EventSendError, EventSink};
use crate::api::events::Event;
use crate::model::progress::{
    ProgressPhase, ProgressScope, ProgressSnapshot, ProgressStatus, ProgressUnit,
};
use crate::model::request::RequestId;
use crate::model::session::SessionId;

fn progress(scope: &ProgressScope, status: ProgressStatus, done: usize) -> Event {
    Event::ProgressUpdated {
        scope: scope.clone(),
        snapshot: ProgressSnapshot::new(
            status,
            ProgressPhase::Processing,
            ProgressUnit::Entry,
            done,
            Some(100),
            None,
        ),
    }
}

#[tokio::test]
async fn runtime_event_queue_uses_the_default_bounded_capacity() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime(tracker.clone());

    assert_eq!(receiver.capacity(), Some(DEFAULT_EVENT_CHANNEL_CAPACITY));
    tracker.shutdown().await.unwrap();
    assert!(sink.send(Event::SessionCreated(SessionId::new())).is_err());
}

#[tokio::test]
async fn progress_updates_coalesce_but_terminal_state_is_preserved() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 1);
    let scope = ProgressScope::scan(SessionId::new(), RequestId::new());

    for done in 0..100 {
        sink.send(progress(&scope, ProgressStatus::Running, done))
            .unwrap();
    }
    sink.send(progress(&scope, ProgressStatus::Completed, 100))
        .unwrap();
    assert_eq!(sink.buffered_state_counts().0, 0);
    sink.send(progress(&scope, ProgressStatus::Running, 101))
        .unwrap();

    let mut received = Vec::new();
    while received.len() < 3 {
        let Ok(event) = tokio::time::timeout(Duration::from_secs(1), receiver.recv_async()).await
        else {
            break;
        };
        let Ok(event) = event else { break };
        received.push(event);
        if received.iter().any(|event| {
            matches!(
                event,
                Event::ProgressUpdated { snapshot, .. }
                    if snapshot.status == ProgressStatus::Completed
            )
        }) {
            break;
        }
    }

    assert!(received.len() <= 2);
    assert!(received.iter().any(|event| {
        matches!(
            event,
            Event::ProgressUpdated { snapshot, .. }
                if snapshot.status == ProgressStatus::Completed && snapshot.done == 100
        )
    }));
    assert!(!received.iter().any(|event| {
        matches!(
            event,
            Event::ProgressUpdated { snapshot, .. }
                if snapshot.status == ProgressStatus::Running && snapshot.done == 101
        )
    }));
    tracker.shutdown().await.unwrap();
}

#[tokio::test]
async fn progress_and_terminal_scope_state_stays_bounded() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 2);

    for done in 0..100 {
        let scope = ProgressScope::scan(SessionId::new(), RequestId::new());
        sink.send(progress(&scope, ProgressStatus::Running, done))
            .unwrap();
    }
    assert!(sink.buffered_state_counts().0 <= 2);

    let drain = tokio::spawn(async move {
        for _ in 0..100 {
            receiver.recv_async().await.unwrap();
        }
    });
    for done in 0..100 {
        let scope = ProgressScope::scan(SessionId::new(), RequestId::new());
        sink.send_async(progress(&scope, ProgressStatus::Completed, done))
            .await
            .unwrap();
    }
    drain.await.unwrap();

    assert!(sink.buffered_state_counts().1 <= 2);
    tracker.shutdown().await.unwrap();
}

#[tokio::test]
async fn lossless_events_eventually_block_when_client_does_not_drain() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 1);
    let mut producer = tokio::spawn(async move {
        for value in 0..10 {
            sink.send_async(Event::SessionCreated(SessionId(value)))
                .await
                .map_err(|_| ())?;
        }
        Ok::<(), ()>(())
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut producer)
            .await
            .is_err()
    );

    for _ in 0..10 {
        tokio::time::timeout(Duration::from_secs(1), receiver.recv_async())
            .await
            .expect("lossless event should be delivered")
            .expect("event hub should remain open");
    }
    assert!(producer.await.unwrap().is_ok());
    tracker.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_a_hub_blocked_by_a_full_client_queue() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 1);

    sink.send_async(Event::SessionCreated(SessionId(1)))
        .await
        .unwrap();
    while receiver.is_empty() {
        tokio::task::yield_now().await;
    }
    sink.send_async(Event::SessionCreated(SessionId(2)))
        .await
        .unwrap();
    tokio::task::yield_now().await;

    tokio::time::timeout(Duration::from_secs(1), tracker.shutdown())
        .await
        .expect("shutdown should cancel the blocked event hub")
        .unwrap();
}

#[tokio::test]
async fn small_queue_retains_errors_and_all_terminal_progress_states() {
    let tracker = WorkTracker::new();
    let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 1);
    let session = SessionId::new();
    let request = RequestId::new();
    let scope = ProgressScope::scan(session, request);
    let producer = tokio::spawn(async move {
        for status in [
            ProgressStatus::Completed,
            ProgressStatus::Cancelled,
            ProgressStatus::Failed,
        ] {
            sink.send_async(progress(&scope, status, 100))
                .await
                .unwrap();
        }
        sink.send_async(Event::from_request_error(
            crate::CoreError::invalid_input("retained error"),
            session,
            request,
        ))
        .await
        .unwrap();
    });
    for expected in [
        ProgressStatus::Completed,
        ProgressStatus::Cancelled,
        ProgressStatus::Failed,
    ] {
        let event = tokio::time::timeout(Duration::from_secs(1), receiver.recv_async())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(event, Event::ProgressUpdated { snapshot, .. } if snapshot.status == expected)
        );
    }
    let event = tokio::time::timeout(Duration::from_secs(1), receiver.recv_async())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, Event::Error { session: actual, request: Some(actual_request), .. }
        if actual == session && actual_request == request)
    );
    producer.await.unwrap();
    tracker.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_and_disconnection_release_senders_waiting_for_capacity() {
    for disconnect in [false, true] {
        let tracker = WorkTracker::new();
        let (sink, receiver) = EventSink::for_runtime_with_capacity(tracker.clone(), 1);
        let mut producer = tokio::spawn(async move {
            for value in 0..10 {
                sink.send_async(Event::SessionCreated(SessionId(value)))
                    .await?;
            }
            Ok::<(), EventSendError>(())
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut producer)
                .await
                .is_err()
        );
        if disconnect {
            drop(receiver);
        } else {
            tokio::time::timeout(Duration::from_secs(1), tracker.shutdown())
                .await
                .unwrap()
                .unwrap();
        }
        let result = tokio::time::timeout(Duration::from_secs(1), producer)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, Err(EventSendError::Closed));
        tracker.shutdown().await.unwrap();
    }
}
