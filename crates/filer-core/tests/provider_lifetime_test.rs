use std::sync::Arc;
use std::time::Duration;

use filer_core::Event;
use tokio::time::timeout;

#[allow(dead_code)]
#[path = "mutation_queue/common.rs"]
mod common;
mod support;

use common::{NativeCopyGate, TIMEOUT, accepted, copy, core, started};

#[test]
fn shutdown_waits_for_a_queued_local_fs_copy() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        let output = destination.join("source");
        std::fs::write(&source, b"copy contents").unwrap();
        std::fs::create_dir(&destination).unwrap();
        let (release, held) = flume::bounded(1);
        let mut provider = common::CopyGate::new(0);
        provider.native = Some(NativeCopyGate {
            queued: tokio::sync::Notify::new(),
            release: held,
        });
        let provider = Arc::new(provider);
        let (core, events) = core(provider.clone());
        let session = support::create_session(&core, &events, TIMEOUT).await;
        let copy = copy(session, &[&source], &destination);
        core.send(copy.clone()).unwrap();
        accepted(&events, &copy).await;
        started(&events, &copy).await;
        timeout(TIMEOUT, provider.native.as_ref().unwrap().queued.notified())
            .await
            .unwrap();

        let consume = tokio::spawn(async move {
            let mut terminal = Vec::new();
            while let Ok(event) = events.recv_async().await {
                if !matches!(event, Event::ProgressUpdated { .. }) {
                    terminal.push(event);
                }
            }
            terminal
        });
        let shutdown = core.shutdown();
        tokio::pin!(shutdown);
        assert!(
            timeout(Duration::from_millis(25), &mut shutdown)
                .await
                .is_err(),
            "shutdown completed while the only blocking worker was occupied"
        );
        assert!(!output.exists());
        release.send(()).unwrap();
        timeout(TIMEOUT, shutdown).await.unwrap().unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"copy contents");

        std::fs::remove_file(&output).unwrap();
        tokio::task::spawn_blocking(|| {}).await.unwrap();
        assert!(!output.exists());
        let terminal = timeout(TIMEOUT, consume).await.unwrap().unwrap();
        assert!(matches!(terminal.as_slice(), [
            Event::OperationComplete { operation_id, success: true, .. },
            Event::SessionDestroyed(actual)
        ] if Some(*operation_id) == copy.operation_id() && *actual == session));
    });
}
