use std::collections::HashSet;
use std::time::Duration;

use filer_core::api::event_sink::DEFAULT_EVENT_CHANNEL_CAPACITY;
use filer_core::{Command, Event, FilerCore};
use tokio::time::timeout;

#[tokio::test(flavor = "current_thread")]
async fn handshake_backpressure_keeps_consumer_and_timer_running() {
    const CASE: &str = "handshake_backpressure_keeps_consumer_and_timer_running";
    if std::env::var("FILER_DELIVERY_CASE").as_deref() != Ok(CASE) {
        // The deadline must run outside the executor that a blocking send can freeze.
        let output = timeout(
            Duration::from_secs(10),
            tokio::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", CASE, "--nocapture"])
                .env("FILER_DELIVERY_CASE", CASE)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("lossless delivery froze the current-thread executor")
        .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let core = FilerCore::new();
    let events = core.event_receiver();
    let count = DEFAULT_EVENT_CHANNEL_CAPACITY * 4;
    let consumer = tokio::spawn(async move {
        let mut sessions = HashSet::new();
        for _ in 0..count {
            match events.recv_async().await.unwrap() {
                Event::SessionCreated(session) => assert!(sessions.insert(session)),
                other => panic!("expected SessionCreated, got {other:?}"),
            }
        }
        assert_eq!(sessions.len(), count);
    });
    let timer = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
    });
    for _ in 0..count {
        core.send(Command::Handshake).unwrap();
    }
    consumer.await.unwrap();
    timer.await.unwrap();
    core.shutdown().await.unwrap();
}
