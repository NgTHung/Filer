use std::sync::Arc;
use std::sync::atomic::Ordering;

use filer_core::model::session::SessionId;
use filer_core::modules::preview::PreviewModule;
use filer_core::{
    Command, ErrorCode, Event, LocalFs, OperationId, OperationQueueResolution, RequestId,
    WireCommand,
};
use tokio::time::timeout;

#[path = "mutation_queue/common.rs"]
mod common;
mod support;

use common::*;

fn delete(session: SessionId, source: &std::path::Path) -> Command {
    Command::Delete {
        locations: vec![local_ref(source)],
        trash: false,
        session,
        request: RequestId::new(),
        operation: OperationId::new(),
    }
}

fn resolve(session: SessionId, failed: &Command, resolution: OperationQueueResolution) -> Command {
    Command::ResolveOperationQueue {
        session,
        request: RequestId::new(),
        failed: failed.operation_id().unwrap(),
        resolution,
    }
}

async fn paused(events: &flume::Receiver<Event>, failed: &Command, waiting: &[OperationId]) {
    let event = next(events).await;
    assert!(
        matches!(event, Event::OperationQueuePaused { session, failed: operation, ref queued }
        if Some(session) == failed.session_id()
            && Some(operation) == failed.operation_id()
            && queued.as_slice() == waiting),
        "{event:?}"
    );
}

async fn failed(
    events: &flume::Receiver<Event>,
    command: &Command,
) -> Vec<filer_core::LocationRef> {
    match next(events).await {
        Event::Error {
            code: ErrorCode::IoFailed,
            request,
            operation,
            affected: Some(affected),
            ..
        } if request == command.request_id() && operation == command.operation_id() => affected,
        event => panic!("expected correlated failure, got {event:?}"),
    }
}

#[tokio::test]
async fn partial_copy_pauses_delete_until_continue_even_after_view_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let copied = root.path().join("copied.txt");
    let gate = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&copied, b"copied").unwrap();
    std::fs::write(&gate, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(1));
    let (core, events) = core(provider.clone());
    core.load(PreviewModule::new(Arc::new(LocalFs::new())));
    let session = support::create_session(&core, &events, TIMEOUT).await;
    let other = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&copied, &gate], &destination);
    let delete = delete(session, &gate);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();
    core.send(delete.clone()).unwrap();
    accepted(&events, &delete).await;

    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    assert_eq!(
        failed(&events, &copy).await,
        vec![local_ref(&destination.join("copied.txt"))]
    );
    let waiting = delete.operation_id().unwrap();
    paused(&events, &copy, &[waiting]).await;

    core.send(Command::CancelPreview { session }).unwrap();
    let inspect = RequestId::new();
    core.send(Command::InspectOperationQueue {
        session,
        request: inspect,
    })
    .unwrap();
    assert!(matches!(next(&events).await, Event::OperationQueueState {
        session: actual, request, running: None, paused_on, queued,
    } if actual == session && request == inspect && paused_on == copy.operation_id() && queued == [waiting]));

    let read = RequestId::new();
    core.send(Command::LoadMetadata {
        session,
        request: read,
        location: local_ref(&gate),
    })
    .unwrap();
    assert!(
        matches!(next(&events).await, Event::MetadataLoaded { session: actual, request, .. }
        if actual == session && request == read)
    );
    let independent = create(other, root.path(), "independent");
    core.send(independent.clone()).unwrap();
    accepted(&events, &independent).await;
    started(&events, &independent).await;
    complete(&events, &independent).await;
    assert_eq!(provider.deletes.load(Ordering::Relaxed), 0);
    assert_eq!(std::fs::read(&gate).unwrap(), b"source");

    let decision = resolve(session, &copy, OperationQueueResolution::Continue);
    core.send(decision.clone()).unwrap();
    assert!(matches!(next(&events).await, Event::OperationQueueState {
        request, paused_on: None, ..
    } if Some(request) == decision.request_id()));
    started(&events, &delete).await;
    complete(&events, &delete).await;
    assert!(!gate.exists());
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn retry_validates_partial_state_and_repeated_failure_can_cancel_the_remainder() {
    let root = tempfile::tempdir().unwrap();
    let copied = root.path().join("copied.txt");
    let gate = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&copied, b"copied").unwrap();
    std::fs::write(&gate, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(2));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&copied, &gate], &destination);
    let delete = delete(session, &gate);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();
    core.send(delete.clone()).unwrap();
    accepted(&events, &delete).await;
    let mut waiting = vec![delete.operation_id().unwrap()];
    for index in 2..filer_core::modules::operations::DEFAULT_OPERATION_QUEUE_CAPACITY {
        let command = create(session, root.path(), &format!("queued-{index}"));
        core.send(command.clone()).unwrap();
        accepted(&events, &command).await;
        waiting.push(command.operation_id().unwrap());
    }
    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    failed(&events, &copy).await;
    paused(&events, &copy, &waiting).await;

    let blocked_retry = resolve(
        session,
        &copy,
        OperationQueueResolution::Retry {
            operation: OperationId::new(),
        },
    );
    core.send(blocked_retry.clone()).unwrap();
    assert!(matches!(next(&events).await, Event::Error {
        code: ErrorCode::Collision, request, operation, affected: None, ..
    } if request == blocked_retry.request_id() && operation == blocked_retry.operation_id()));

    std::fs::remove_file(destination.join("copied.txt")).unwrap();
    let retry = resolve(
        session,
        &copy,
        OperationQueueResolution::Retry {
            operation: OperationId::new(),
        },
    );
    let wire = WireCommand::try_from(retry.clone()).unwrap();
    let encoded = serde_json::to_string(&wire).unwrap();
    core.send(Command::from(
        serde_json::from_str::<WireCommand>(&encoded).unwrap(),
    ))
    .unwrap();
    accepted(&events, &retry).await;
    started(&events, &retry).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();
    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    assert_eq!(
        failed(&events, &retry).await,
        vec![local_ref(&destination.join("copied.txt"))]
    );
    paused(&events, &retry, &waiting).await;

    let stale = resolve(session, &copy, OperationQueueResolution::Continue);
    core.send(stale.clone()).unwrap();
    assert!(matches!(next(&events).await, Event::Error {
        code: ErrorCode::OperationQueueStateChanged, request, ..
    } if request == stale.request_id()));

    let cancel = resolve(session, &retry, OperationQueueResolution::Cancel);
    core.send(cancel.clone()).unwrap();
    for id in waiting {
        assert!(matches!(next(&events).await, Event::Error {
            code: ErrorCode::Cancelled, operation: Some(operation), affected: Some(affected), ..
        } if operation == id && affected.is_empty()));
    }
    assert!(matches!(next(&events).await, Event::OperationQueueState {
        request, running: None, paused_on: None, queued, ..
    } if Some(request) == cancel.request_id() && queued.is_empty()));
    assert_eq!(provider.deletes.load(Ordering::Relaxed), 0);
    assert_eq!(std::fs::read(&gate).unwrap(), b"source");
    core.shutdown().await.unwrap();
}

#[tokio::test]
async fn successful_retry_runs_before_the_accepted_delete() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("gated.txt");
    let destination = root.path().join("destination");
    std::fs::write(&source, b"source").unwrap();
    std::fs::create_dir(&destination).unwrap();
    let provider = Arc::new(CopyGate::new(1));
    let (core, events) = core(provider.clone());
    let session = support::create_session(&core, &events, TIMEOUT).await;

    let copy = copy(session, &[&source], &destination);
    let delete = delete(session, &source);
    core.send(copy.clone()).unwrap();
    accepted(&events, &copy).await;
    started(&events, &copy).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();
    core.send(delete.clone()).unwrap();
    accepted(&events, &delete).await;

    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    assert!(failed(&events, &copy).await.is_empty());
    paused(&events, &copy, &[delete.operation_id().unwrap()]).await;

    let retry = resolve(
        session,
        &copy,
        OperationQueueResolution::Retry {
            operation: OperationId::new(),
        },
    );
    core.send(retry.clone()).unwrap();
    accepted(&events, &retry).await;
    started(&events, &retry).await;
    timeout(TIMEOUT, provider.entered.wait()).await.unwrap();
    assert_eq!(provider.deletes.load(Ordering::Relaxed), 0);
    timeout(TIMEOUT, provider.release.wait()).await.unwrap();
    complete(&events, &retry).await;
    started(&events, &delete).await;
    complete(&events, &delete).await;
    assert_eq!(
        std::fs::read(destination.join("gated.txt")).unwrap(),
        b"source"
    );
    assert!(!source.exists());
    core.shutdown().await.unwrap();
}
