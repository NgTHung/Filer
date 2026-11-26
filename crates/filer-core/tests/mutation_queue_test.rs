use std::sync::Arc;

use filer_core::modules::operations::DEFAULT_OPERATION_QUEUE_CAPACITY;
use filer_core::{Command, ErrorCode, Event, OperationId};
use tokio::time::timeout;

#[path = "mutation_queue/common.rs"]
mod common;
mod support;

use common::*;

#[tokio::test]
async fn held_copy_preserves_fifo_and_other_sessions_progress() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&source, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(0));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;
    let other = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&source], &destination);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();

    let first = create(session, root.path(), "first");
    let second = create(session, &root.path().join("first"), "second");
    for command in [&first, &second] {
        core.send(command.clone()).unwrap();
        accepted(&events, command).await;
    }
    let independent = create(other, root.path(), "independent");
    core.send(independent.clone()).unwrap();
    accepted(&events, &independent).await;
    started(&events, &independent).await;
    complete(&events, &independent).await;
    assert!(!root.path().join("first").exists());

    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    complete(&events, &copy).await;
    for command in [&first, &second] {
        started(&events, command).await;
        complete(&events, command).await;
    }
    assert_eq!(
        std::fs::read(destination.join("gated.txt")).unwrap(),
        b"source"
    );
    assert!(root.path().join("first/second").is_dir());
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn full_admission_rejects_work_and_keeps_controls_serviceable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&source, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(0));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&source], &destination);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();

    let mut waiting = Vec::new();
    for index in 1..DEFAULT_OPERATION_QUEUE_CAPACITY {
        let command = create(session, root.path(), &format!("queued-{index}"));
        core.send(command.clone()).unwrap();
        accepted(&events, &command).await;
        waiting.push(command.operation_id().unwrap());
    }

    let busy = create(session, root.path(), "busy");
    core.send(busy.clone()).unwrap();
    assert!(matches!(next(&events).await, Event::Error {
        code: ErrorCode::OperationQueueFull, session: actual, request, operation, ..
    } if actual == session && request == busy.request_id() && operation == busy.operation_id()));

    let cancelled = waiting.pop().unwrap();
    core.send(Command::CancelOperation {
        session,
        operation: cancelled,
    })
    .unwrap();
    assert_cancelled(&events, cancelled).await;
    let replacement = create(session, root.path(), "replacement");
    core.send(replacement.clone()).unwrap();
    accepted(&events, &replacement).await;

    core.send(Command::DestroySession(session)).unwrap();
    let during_close = create(session, root.path(), "while-closing");
    core.send(during_close.clone()).unwrap();
    assert!(matches!(next(&events).await, Event::Error {
        code: ErrorCode::SessionClosing, session: actual, request, operation, ..
    } if actual == session && request == during_close.request_id()
        && operation == during_close.operation_id()));
    assert!(core.sessions().exists(session));

    let cancelled = waiting.pop().unwrap();
    core.send(Command::CancelOperation {
        session,
        operation: cancelled,
    })
    .unwrap();
    assert_cancelled(&events, cancelled).await;
    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    complete(&events, &copy).await;
    waiting.push(replacement.operation_id().unwrap());
    for operation in waiting {
        assert!(matches!(next(&events).await, Event::OperationStarted {
            operation: actual, ..
        } if actual == operation));
        assert!(matches!(next(&events).await, Event::ProgressUpdated { .. }));
        assert!(matches!(next(&events).await, Event::OperationComplete {
            operation_id, session: actual, success: true, ..
        } if actual == session && operation_id == operation));
    }
    assert!(matches!(next(&events).await, Event::SessionDestroyed(actual) if actual == session));
    assert!(!core.sessions().exists(session));
    assert!(!root.path().join("busy").exists());
    assert!(!root.path().join("while-closing").exists());
    assert!(root.path().join("replacement").is_dir());
    assert_eq!(
        std::fs::read(destination.join("gated.txt")).unwrap(),
        b"source"
    );
    core.shutdown().await.unwrap();
}

async fn assert_cancelled(events: &flume::Receiver<Event>, id: OperationId) {
    let event = timeout(TIMEOUT, async {
        loop {
            let event = next(events).await;
            if !matches!(event, Event::ProgressUpdated { .. }) {
                return event;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(event, Event::Error {
        code: ErrorCode::Cancelled, operation: Some(operation), ..
    } if operation == id),
        "{event:?}"
    );
}

#[tokio::test]
async fn transfer_rechecks_a_destination_that_changed_after_admission() {
    let root = tempfile::tempdir().unwrap();
    let gate = root.path().join("gated.txt");
    let source = root.path().join("second.txt");
    let destination = root.path().join("destination");
    std::fs::write(&gate, b"gate").unwrap();
    std::fs::write(&source, b"preserve this source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(0));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;

    let first = copy(session, &[&gate], &destination);
    core.send(first.clone()).unwrap();
    accepted(&events, &first).await;
    started(&events, &first).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();

    let second = copy(session, &[&source], &destination);
    core.send(second.clone()).unwrap();
    accepted(&events, &second).await;
    std::fs::hard_link(&source, destination.join("second.txt")).unwrap();

    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    complete(&events, &first).await;
    started(&events, &second).await;
    assert!(matches!(next(&events).await, Event::Error {
        code: ErrorCode::InputInvalid, request, operation, ..
    } if request == second.request_id() && operation == second.operation_id()));
    assert_eq!(std::fs::read(&source).unwrap(), b"preserve this source");
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancel_operation_interrupts_the_matching_running_copy() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&source, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(0));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&source], &destination);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();

    let operation = copy.operation_id().unwrap();
    core.send(Command::CancelOperation { session, operation })
        .unwrap();
    assert_cancelled(&events, operation).await;
    assert!(!destination.join("gated.txt").exists());
    core.shutdown().await.unwrap();
}
