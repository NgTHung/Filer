use std::sync::Arc;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::fs_change::FsChangeKind;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::modules::watch::watcher::{WatchCommand, Watcher};
use crate::vfs::local_watch::LocalWatchProvider;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

/// Helper to wait for and collect events with timeout
async fn collect_events(
    rx: &flume::Receiver<Event>,
    expected_count: usize,
    timeout_ms: u64,
) -> Vec<Event> {
    let mut events = Vec::new();
    let deadline = Duration::from_millis(timeout_ms);

    for _ in 0..expected_count {
        match timeout(deadline, rx.recv_async()).await {
            Ok(Ok(event)) => events.push(event),
            _ => break,
        }
    }

    events
}

/// Helper to collect all available events without blocking
fn collect_available_events(rx: &flume::Receiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// Helper to create a Watcher with a LocalWatchProvider
fn setup_watcher(
    cmd_rx: flume::Receiver<WatchCommand>,
    evt_tx: flume::Sender<Event>,
    registry: NodeRegistry,
) -> Watcher {
    let provider = Arc::new(LocalWatchProvider::new());
    Watcher::new(cmd_rx, evt_tx, registry, provider)
}

#[tokio::test]
async fn test_watch_command() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Create a file in the watched directory
    let test_file = test_path.join("test.txt");
    fs::write(&test_file, "hello").unwrap();

    let events = collect_events(&evt_rx, 1, 2000).await;

    assert!(!events.is_empty(), "Should receive at least one event");

    let has_create_event = events.iter().any(|e| {
        matches!(
            e,
            Event::FsChanged {
                kind: FsChangeKind::Created,
                session,
                ..
            } if *session == SessionId(1)
        )
    });

    assert!(has_create_event, "Should receive FsChanged::Created event");

    // Cleanup
    cmd_tx.send(WatchCommand::UnwatchAll).unwrap();
    sleep(Duration::from_millis(50)).await;
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_unwatch_command() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Start watching
    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Create a file (should trigger event)
    let test_file = test_path.join("before_unwatch.txt");
    fs::write(&test_file, "before").unwrap();

    let events = collect_events(&evt_rx, 1, 2000).await;
    assert!(!events.is_empty(), "Should receive event before unwatch");

    // Clear any remaining events
    collect_available_events(&evt_rx);

    // Unwatch
    cmd_tx.send(WatchCommand::Unwatch(test_node)).unwrap();
    sleep(Duration::from_millis(100)).await;

    // Create another file (should NOT trigger event)
    let test_file2 = test_path.join("after_unwatch.txt");
    fs::write(&test_file2, "after").unwrap();

    // Wait and verify no events
    let events = collect_events(&evt_rx, 1, 500).await;
    assert!(events.is_empty(), "Should not receive events after unwatch");

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_fs_changed_create() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Create file
    let test_file = test_path.join("created.txt");
    fs::write(&test_file, "content").unwrap();

    let events = collect_events(&evt_rx, 1, 2000).await;

    assert!(!events.is_empty());
    let has_create = events.iter().any(|e| {
        matches!(
            e,
            Event::FsChanged {
                kind: FsChangeKind::Created,
                ..
            }
        )
    });
    assert!(has_create, "Should receive FsChanged::Created");

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_fs_changed_modify() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    // Create file before watching
    let test_file = test_path.join("modify_me.txt");
    fs::write(&test_file, "initial").unwrap();

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Clear any initial events
    collect_available_events(&evt_rx);

    // Modify file
    fs::write(&test_file, "modified content").unwrap();

    let events = collect_events(&evt_rx, 1, 2000).await;

    assert!(!events.is_empty(), "Should receive modification event");
    let has_modify = events.iter().any(|e| {
        matches!(
            e,
            Event::FsChanged {
                kind: FsChangeKind::Modified,
                ..
            }
        )
    });
    assert!(has_modify, "Should receive FsChanged::Modified");

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_fs_changed_delete() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    // Create file before watching
    let test_file = test_path.join("delete_me.txt");
    fs::write(&test_file, "content").unwrap();

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Clear any initial events
    collect_available_events(&evt_rx);

    // Delete file
    fs::remove_file(&test_file).unwrap();

    let events = collect_events(&evt_rx, 1, 2000).await;

    assert!(!events.is_empty(), "Should receive deletion event");
    let has_delete = events.iter().any(|e| {
        matches!(
            e,
            Event::FsChanged {
                kind: FsChangeKind::Deleted,
                ..
            }
        )
    });
    assert!(has_delete, "Should receive FsChanged::Deleted");

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_debouncing_rapid_changes() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    let test_file = test_path.join("rapid_changes.txt");

    // Make rapid changes
    for i in 0..10 {
        fs::write(&test_file, format!("change {}", i)).unwrap();
        sleep(Duration::from_millis(5)).await;
    }

    // Wait for debouncing period to complete (timeout is 1s, tick is 100ms)
    sleep(Duration::from_millis(1500)).await;

    // Collect all events
    let events = collect_available_events(&evt_rx);

    assert!(
        events.len() < 10,
        "Debouncing should reduce event count. Got {} events for 10 rapid changes",
        events.len()
    );

    assert!(
        !events.is_empty(),
        "Should still receive some debounced events"
    );

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_unwatch_session() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();
    let path1 = temp_dir1.path().to_path_buf();
    let path2 = temp_dir2.path().to_path_buf();

    let node1 = registry.clone().register(path1.clone());
    let node2 = registry.clone().register(path2.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Watch two directories with same session
    cmd_tx
        .send(WatchCommand::Watch(node1, SessionId(1)))
        .unwrap();
    cmd_tx
        .send(WatchCommand::Watch(node2, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Unwatch entire session
    cmd_tx
        .send(WatchCommand::UnwatchSession(SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Clear any events
    collect_available_events(&evt_rx);

    // Create files in both directories
    fs::write(path1.join("test1.txt"), "content1").unwrap();
    fs::write(path2.join("test2.txt"), "content2").unwrap();

    // Wait and verify no events
    let events = collect_events(&evt_rx, 1, 500).await;
    assert!(
        events.is_empty(),
        "Should not receive events after session unwatch"
    );

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_multiple_sessions_watching_same_path() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Two sessions watch the same path
    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(2)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Create a file
    let test_file = test_path.join("shared.txt");
    fs::write(&test_file, "content").unwrap();

    // Wait for events
    let events = collect_events(&evt_rx, 2, 2000).await;

    // Both sessions should receive the event
    let session1_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::FsChanged { session, .. } if *session == SessionId(1)))
        .collect();

    let session2_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::FsChanged { session, .. } if *session == SessionId(2)))
        .collect();

    assert!(
        !session1_events.is_empty(),
        "Session 1 should receive event"
    );
    assert!(
        !session2_events.is_empty(),
        "Session 2 should receive event"
    );

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_watch_subdirectories() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    // Create subdirectory
    let subdir = test_path.join("subdir");
    fs::create_dir(&subdir).unwrap();

    let test_node = registry.clone().register(test_path.clone());

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch(test_node, SessionId(1)))
        .unwrap();
    sleep(Duration::from_millis(100)).await;

    // Clear any initial events from directory creation
    collect_available_events(&evt_rx);

    // Create file in subdirectory
    let test_file = subdir.join("nested.txt");
    fs::write(&test_file, "nested content").unwrap();

    // Should receive event for nested file
    let events = collect_events(&evt_rx, 1, 2000).await;

    assert!(
        !events.is_empty(),
        "Should receive events for files in subdirectories"
    );

    // Cleanup
    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}
