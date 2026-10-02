use std::collections::HashSet;
use std::time::Duration;

use filer_core::api::event_sink::DEFAULT_EVENT_CHANNEL_CAPACITY;
use filer_core::model::directory::DirectoryLoadOptions;
use filer_core::{
    Command, Event, FilerCore, LocationId, LocationRef, OperationId, PipelineConfig, RequestId,
};
use tokio::time::timeout;

async fn run_isolated(case: &str, family: &str) {
    // The deadline must run outside the executor that a blocking send can freeze.
    let output = timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env("FILER_DELIVERY_CASE", case)
            .env("FILER_DELIVERY_FAMILY", family)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap_or_else(|_| panic!("{family}: lossless delivery froze the current-thread executor"))
    .unwrap();
    assert!(
        output.status.success(),
        "{family}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn handshake_backpressure_keeps_consumer_and_timer_running() {
    const CASE: &str = "handshake_backpressure_keeps_consumer_and_timer_running";
    if std::env::var("FILER_DELIVERY_CASE").as_deref() != Ok(CASE) {
        run_isolated(CASE, "handshake").await;
        return;
    }

    let core = FilerCore::new();
    let events = core.event_receiver();
    let count = DEFAULT_EVENT_CHANNEL_CAPACITY * 4;
    let (timer_tx, timer_rx) = tokio::sync::oneshot::channel();
    let consumer = tokio::spawn(async move {
        let mut sessions = HashSet::new();
        let first = events.recv_async().await.unwrap();
        assert!(matches!(first, Event::SessionCreated(session) if sessions.insert(session)));
        timer_rx.await.unwrap();
        for _ in 1..count {
            match events.recv_async().await.unwrap() {
                Event::SessionCreated(session) => assert!(sessions.insert(session)),
                other => panic!("expected SessionCreated, got {other:?}"),
            }
        }
        assert_eq!(sessions.len(), count);
    });
    let timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        timer_tx.send(()).unwrap();
    });
    for _ in 0..count {
        core.send(Command::Handshake).unwrap();
    }
    consumer.await.unwrap();
    timer.await.unwrap();
    core.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn actor_errors_remain_correlated_under_backpressure() {
    const CASE: &str = "actor_errors_remain_correlated_under_backpressure";
    if std::env::var("FILER_DELIVERY_CASE").as_deref() != Ok(CASE) {
        for family in [
            "navigate",
            "scan",
            "search",
            "query",
            "preview",
            "watch",
            "copy",
            "move",
            "delete",
            "rename",
            "create_file",
            "create_folder",
        ] {
            run_isolated(CASE, family).await;
        }
        return;
    }
    let core = FilerCore::with_defaults();
    let events = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    let session = match events.recv_async().await.unwrap() {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    };
    let count = DEFAULT_EVENT_CHANNEL_CAPACITY * 4;
    let consumer = tokio::spawn(async move {
        let mut requests = HashSet::new();
        for _ in 0..count {
            match events.recv_async().await.unwrap() {
                Event::Error {
                    session: actual,
                    request: Some(request),
                    ..
                } => {
                    assert_eq!(actual, session);
                    assert!(requests.insert(request));
                }
                other => panic!("expected correlated Error, got {other:?}"),
            }
        }
    });
    let family = std::env::var("FILER_DELIVERY_FAMILY").unwrap();
    for _ in 0..count {
        let location = LocationRef::id_only(LocationId(99999));
        let request = RequestId::new();
        let operation = OperationId::new();
        let command = match family.as_str() {
            "navigate" => Command::Navigate {
                location,
                session,
                request,
            },
            "scan" => Command::Scan {
                location,
                session,
                request,
                pipeline: PipelineConfig::default(),
                load: DirectoryLoadOptions::default(),
            },
            "search" | "query" => Command::Search {
                root: location,
                session,
                request,
                query: if family == "query" {
                    "type:unknown"
                } else {
                    "needle"
                }
                .into(),
            },
            "preview" => Command::LoadPreview {
                location,
                session,
                request,
                options: None,
            },
            "watch" => Command::Watch {
                location,
                session,
                request,
            },
            "copy" => Command::Copy {
                sources: vec![],
                destination: location,
                session,
                request,
                operation,
            },
            "move" => Command::Move {
                sources: vec![],
                destination: location,
                session,
                request,
                operation,
            },
            "delete" => Command::Delete {
                locations: vec![location],
                trash: false,
                session,
                request,
                operation,
            },
            "rename" => Command::Rename {
                location,
                new_name: "name".into(),
                session,
                request,
                operation,
            },
            "create_file" => Command::CreateFile {
                parent: location,
                name: "name".into(),
                session,
                request,
                operation,
            },
            "create_folder" => Command::CreateFolder {
                parent: location,
                name: "name".into(),
                session,
                request,
                operation,
            },
            other => panic!("unknown family {other}"),
        };
        core.send(command).unwrap();
    }
    consumer.await.unwrap();
    core.shutdown().await.unwrap();
}
