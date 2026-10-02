use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use filer_core::model::session::SessionId;
use filer_core::modules::operations::OperationsModule;
use filer_core::{
    Command, ErrorCode, Event, FilerCore, LocalFs, Location, LocationRef, OperationId, RequestId,
};
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(10);
const CONTENT: &[u8] = b"source bytes must survive rejection";

async fn isolated_case(name: &str) -> Option<tempfile::TempDir> {
    if std::env::var("FILER_TRANSFER_CASE").as_deref() == Ok(name) {
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    let output = timeout(
        TIMEOUT,
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("FILER_TRANSFER_CASE", name)
            .env("FILER_TRANSFER_ROOT", root.path())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("isolated transfer exceeded its deadline")
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Some(root)
}

fn local_ref(path: &Path) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

async fn transfer(sources: &[&Path], destination: &Path, moving: bool) -> Event {
    let core = FilerCore::new();
    core.load(OperationsModule::new(Arc::new(LocalFs::new())));
    let events = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    let session = match timeout(TIMEOUT, events.recv_async())
        .await
        .unwrap()
        .unwrap()
    {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    };
    let request = RequestId::new();
    let operation = OperationId::new();
    let sources = sources.iter().map(|path| local_ref(path)).collect();
    let destination = local_ref(destination);
    let command = if moving {
        Command::Move {
            sources,
            destination,
            session,
            request,
            operation,
        }
    } else {
        Command::Copy {
            sources,
            destination,
            session,
            request,
            operation,
        }
    };
    core.send(command).unwrap();
    timeout(TIMEOUT, async {
        loop {
            match events.recv_async().await.unwrap() {
                event @ Event::Error { .. } => {
                    assert_rejection(&event, session, request, operation);
                    return event;
                }
                event @ Event::OperationComplete { .. } => return event,
                _ => {}
            }
        }
    })
    .await
    .unwrap()
}

fn assert_rejection(event: &Event, session: SessionId, request: RequestId, operation: OperationId) {
    assert!(matches!(event, Event::Error {
        code: ErrorCode::InputInvalid,
        session: actual_session,
        request: Some(actual_request),
        operation: Some(actual_operation),
        ..
    } if *actual_session == session && *actual_request == request && *actual_operation == operation));
}

#[tokio::test]
async fn copy_rejects_own_parent_without_truncation() {
    if isolated_case("copy_rejects_own_parent_without_truncation")
        .await
        .is_some()
    {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("FILER_TRANSFER_ROOT").unwrap());
    let source = root.join("file.txt");
    std::fs::write(&source, CONTENT).unwrap();
    assert!(matches!(
        transfer(&[&source], &root, false).await,
        Event::Error { .. }
    ));
    assert_eq!(std::fs::read(source).unwrap(), CONTENT);
}
