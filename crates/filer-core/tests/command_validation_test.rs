use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use filer_core::model::session::SessionId;
use filer_core::modules::operations::OperationsModule;
use filer_core::{
    Command, ErrorCode, Event, FilerCore, LocalFs, Location, LocationRef, OperationId, RequestId,
};
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(5);

async fn create_session(core: &FilerCore, events: &flume::Receiver<Event>) -> SessionId {
    core.send(Command::Handshake).unwrap();
    match timeout(TIMEOUT, events.recv_async())
        .await
        .unwrap()
        .unwrap()
    {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    }
}

fn local_ref(path: &Path) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

async fn terminal_event(events: &flume::Receiver<Event>) -> Event {
    timeout(TIMEOUT, async {
        loop {
            match events.recv_async().await.unwrap() {
                event @ (Event::Error { .. } | Event::OperationComplete { .. }) => return event,
                _ => {}
            }
        }
    })
    .await
    .expect("expected a command outcome")
}

fn assert_error(command: &Command, event: Event, expected_code: ErrorCode) {
    assert!(
        matches!(&event, Event::Error { code, session, request, operation, .. }
            if *code == expected_code
                && Some(*session) == command.session_id()
                && *request == command.request_id()
                && *operation == command.operation_id()),
        "expected a correlated {expected_code:?} for {command:?}, got {event:?}"
    );
}

#[tokio::test]
async fn create_and_rename_reject_invalid_native_names_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    std::fs::create_dir_all(parent.join("nested")).unwrap();
    let source = parent.join("original.txt");
    std::fs::write(&source, b"original bytes").unwrap();
    let rooted = root.path().join("rooted.txt");
    let mut names = vec![
        "../escaped.txt",
        "",
        ".",
        "..",
        "nested/file.txt",
        "trailing/",
        "nul\0byte",
        rooted.to_str().unwrap(),
    ];
    if cfg!(windows) {
        names.extend([
            r"..\escaped.txt",
            r"nested\file.txt",
            "C:relative.txt",
            "name:stream",
            "invalid?name",
            "control\u{1}byte",
            "trailing.",
            "trailing ",
            "NUL.txt",
            "com¹.log",
        ]);
    }

    let core = FilerCore::new();
    core.load(OperationsModule::new(Arc::new(LocalFs::new())));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;

    for name in names {
        for command in [
            Command::CreateFile {
                parent: local_ref(&parent),
                name: name.into(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            },
            Command::CreateFolder {
                parent: local_ref(&parent),
                name: name.into(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            },
            Command::Rename {
                location: local_ref(&source),
                new_name: name.into(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            },
        ] {
            core.send(command.clone()).unwrap();
            assert_error(
                &command,
                terminal_event(&events).await,
                ErrorCode::InputInvalid,
            );
        }
    }

    assert_eq!(std::fs::read(source).unwrap(), b"original bytes");
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(parent.join("nested")).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn create_and_rename_accept_native_names() {
    let root = tempfile::tempdir().unwrap();
    let core = FilerCore::new();
    core.load(OperationsModule::new(Arc::new(LocalFs::new())));
    let events = core.event_receiver();
    let session = create_session(&core, &events).await;

    let original = ".original résumé.txt";
    let renamed = if cfg!(unix) {
        r"native:name\file.txt"
    } else {
        "native name.txt"
    };
    for command in [
        Command::CreateFile {
            parent: local_ref(root.path()),
            name: original.into(),
            session,
            request: RequestId::new(),
            operation: OperationId::new(),
        },
        Command::CreateFolder {
            parent: local_ref(root.path()),
            name: "example folder".into(),
            session,
            request: RequestId::new(),
            operation: OperationId::new(),
        },
        Command::Rename {
            location: local_ref(&root.path().join(original)),
            new_name: renamed.into(),
            session,
            request: RequestId::new(),
            operation: OperationId::new(),
        },
    ] {
        core.send(command.clone()).unwrap();
        let event = terminal_event(&events).await;
        assert!(
            matches!(&event, Event::OperationComplete { operation_id, session: actual, success: true, .. }
                if Some(*operation_id) == command.operation_id() && *actual == session),
            "expected completion for {command:?}, got {event:?}"
        );
    }

    assert!(!root.path().join(original).exists());
    assert!(root.path().join(renamed).is_file());
    assert!(root.path().join("example folder").is_dir());
}
