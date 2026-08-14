use std::sync::Arc;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::model::fs_change::FsChangeKind;
use crate::model::location::{Location, LocationDescriptor, LocationRef, LocationSegment};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::navigation::navigator::NavCommand;
use crate::modules::watch::watcher::{UnwatchScope, WatchCommand, WatchEventMode, Watcher};
use crate::vfs::local_watch::LocalWatchProvider;
use crate::vfs::watch::{FsChange, WatchHandle, WatchProvider};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

fn location_watch(location: LocationRef, session: SessionId) -> WatchCommand {
    WatchCommand::Watch {
        location,
        session,
        request: None,
        event_mode: WatchEventMode::Location,
    }
}

fn unwatch_location(location: LocationRef) -> WatchCommand {
    WatchCommand::Unwatch {
        location,
        scope: UnwatchScope::All,
    }
}

struct TestWatchHandle;

impl WatchHandle for TestWatchHandle {}

#[derive(Default)]
struct TestWatchProvider {
    change_tx: Mutex<Option<flume::Sender<FsChange>>>,
    watched_paths: Mutex<Vec<PathBuf>>,
}

impl TestWatchProvider {
    async fn emit(&self, path: PathBuf, kind: FsChangeKind) {
        let tx = self
            .change_tx
            .lock()
            .unwrap()
            .as_ref()
            .expect("watch should be registered before emitting changes")
            .clone();
        tx.send_async(FsChange { path, kind }).await.unwrap();
    }
}

#[async_trait]
impl WatchProvider for TestWatchProvider {
    async fn watch(
        &self,
        path: &Path,
        tx: flume::Sender<FsChange>,
    ) -> Result<Box<dyn WatchHandle>, crate::errors::CoreError> {
        *self.change_tx.lock().unwrap() = Some(tx);
        self.watched_paths.lock().unwrap().push(path.to_path_buf());
        Ok(Box::new(TestWatchHandle))
    }

    async fn unwatch(&self, path: &Path) -> Result<(), crate::errors::CoreError> {
        self.watched_paths.lock().unwrap().retain(|p| p != path);
        Ok(())
    }
}

#[tokio::test]
async fn test_watch_location_emits_location_event_and_refresh_invalidation() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let (nav_tx, nav_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();
    let location = Location::local(temp_dir.path().to_path_buf());
    let location_ref = LocationRef::from_location(&location);

    let watcher = Watcher::with_refresh(cmd_rx, evt_tx, registry.clone(), provider.clone(), nav_tx);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch {
            location: location_ref.clone(),
            session: SessionId(1),
            request: Some(RequestId::new()),
            event_mode: WatchEventMode::Location,
        })
        .unwrap();
    sleep(Duration::from_millis(20)).await;

    provider
        .emit(temp_dir.path().join("changed.txt"), FsChangeKind::Created)
        .await;

    let events = collect_events(&evt_rx, 1, 100).await;
    assert!(events.iter().any(|event| {
        matches!(
            event,
            Event::FsChanged {
                location,
                kind: FsChangeKind::Created,
                session: SessionId(1),
            } if location == &location_ref
        )
    }));

    assert!(
        matches!(
            timeout(Duration::from_millis(100), nav_rx.recv_async())
                .await
                .expect("location watch should invalidate navigation")
                .expect("nav channel should remain open"),
            NavCommand::Invalidate(_)
        ),
        "direct-local Location watches should bridge to navigation invalidation"
    );

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_watch_location_subscriptions_share_provider_entry() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_path_buf();
    let location = LocationRef::from_location(&Location::local(path.clone()));

    let watcher = Watcher::new(cmd_rx, evt_tx, registry, provider.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch {
            location: location.clone(),
            session: SessionId(1),
            request: Some(RequestId::new()),
            event_mode: WatchEventMode::Location,
        })
        .unwrap();
    cmd_tx
        .send(WatchCommand::Watch {
            location: location.clone(),
            session: SessionId(2),
            request: Some(RequestId::new()),
            event_mode: WatchEventMode::Location,
        })
        .unwrap();
    sleep(Duration::from_millis(20)).await;

    assert_eq!(
        provider.watched_paths.lock().unwrap().len(),
        1,
        "Location subscriptions should share one provider watch"
    );

    provider
        .emit(path.join("changed.txt"), FsChangeKind::Created)
        .await;

    let events = collect_events(&evt_rx, 2, 100).await;
    assert!(events.iter().any(|event| {
        matches!(
            event,
            Event::FsChanged {
                location: event_location,
                kind: FsChangeKind::Created,
                session: SessionId(1),
            } if event_location == &location
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            Event::FsChanged {
                location: event_location,
                kind: FsChangeKind::Created,
                session: SessionId(2),
            } if event_location == &location
        )
    }));

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_watch_location_segmented_route_emits_request_error() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let request = RequestId::new();
    let location =
        LocationRef::Descriptor(LocationDescriptor::local("/tmp/archive.zip").with_segment(
            LocationSegment::ArchiveMember {
                path: PathBuf::from("inner.txt"),
            },
        ));

    let watcher = Watcher::new(cmd_rx, evt_tx, registry, provider);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(WatchCommand::Watch {
            location,
            session: SessionId(1),
            request: Some(request),
            event_mode: WatchEventMode::Location,
        })
        .unwrap();

    let event = timeout(Duration::from_millis(100), evt_rx.recv_async())
        .await
        .expect("segmented watch should emit error")
        .expect("event channel should remain open");

    match event {
        Event::Error {
            request: error_request,
            session,
            ..
        } => {
            assert_eq!(error_request, Some(request));
            assert_eq!(session, SessionId(1));
        }
        other => panic!("Expected Error event, got {other:?}"),
    }

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_watch_command() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Start watching
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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
    cmd_tx
        .send(unwatch_location(LocationRef::from_location(
            &Location::local(test_path.clone()),
        )))
        .unwrap();
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

    // Create file before watching
    let test_file = test_path.join("modify_me.txt");
    fs::write(&test_file, "initial").unwrap();

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

    // Create file before watching
    let test_file = test_path.join("delete_me.txt");
    fs::write(&test_file, "content").unwrap();

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Watch two directories with same session
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(path1.clone())),
            SessionId(1),
        ))
        .unwrap();
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(path2.clone())),
            SessionId(1),
        ))
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    // Two sessions watch the same path
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
        .unwrap();
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(2),
        ))
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

    let watcher = setup_watcher(cmd_rx, evt_tx, registry.clone());
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
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

#[tokio::test]
// API-007 pin: the refresh sink still emits internal NodeId invalidation commands.
async fn test_watcher_refresh_sink_invalidates_once_per_watched_node() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let (nav_tx, nav_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();
    let test_node = registry.clone().register(test_path.clone());
    let watcher = Watcher::with_refresh(cmd_rx, evt_tx, registry.clone(), provider.clone(), nav_tx);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
        .unwrap();
    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(2),
        ))
        .unwrap();
    sleep(Duration::from_millis(20)).await;

    provider
        .emit(test_path.join("changed.txt"), FsChangeKind::Created)
        .await;

    let events = collect_events(&evt_rx, 2, 100).await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::FsChanged { .. }))
            .count(),
        2,
        "both watching sessions should still receive FsChanged"
    );

    let invalidate = timeout(Duration::from_millis(100), nav_rx.recv_async())
        .await
        .expect("watch change should invalidate navigation")
        .expect("nav channel should remain open");
    assert!(
        matches!(invalidate, NavCommand::Invalidate(node) if node == test_node),
        "watch change should invalidate the watched root node"
    );
    assert!(
        nav_rx.try_recv().is_err(),
        "multiple sessions on one watched node should only invalidate once"
    );

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
// API-007 pin: the refresh sink still emits internal NodeId invalidation commands.
async fn test_watcher_refresh_sink_invalidates_for_delete_and_rename() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, _evt_rx) = flume::unbounded();
    let (nav_tx, nav_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_path_buf();
    let test_node = registry.clone().register(test_path.clone());
    let watcher = Watcher::with_refresh(cmd_rx, evt_tx, registry.clone(), provider.clone(), nav_tx);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(test_path.clone())),
            SessionId(1),
        ))
        .unwrap();
    sleep(Duration::from_millis(20)).await;

    provider
        .emit(test_path.join("deleted.txt"), FsChangeKind::Deleted)
        .await;
    provider
        .emit(
            test_path.join("renamed.txt"),
            FsChangeKind::Renamed {
                from: test_path.join("old-name.txt"),
            },
        )
        .await;

    for reason in ["delete", "rename"] {
        let invalidate = timeout(Duration::from_millis(100), nav_rx.recv_async())
            .await
            .unwrap_or_else(|_| panic!("{reason} should invalidate navigation"))
            .expect("nav channel should remain open");
        assert!(
            matches!(invalidate, NavCommand::Invalidate(node) if node == test_node),
            "{reason} should invalidate the watched root node"
        );
    }

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}

#[tokio::test]
async fn test_watcher_refresh_sink_ignores_unrelated_sibling_paths() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let (nav_tx, nav_rx) = flume::unbounded();
    let registry = NodeRegistry::new();
    let provider = Arc::new(TestWatchProvider::default());
    let temp_dir = TempDir::new().unwrap();
    let watched_path = temp_dir.path().join("watched");
    let sibling_path = temp_dir.path().join("watched-sibling");
    fs::create_dir(&watched_path).unwrap();
    fs::create_dir(&sibling_path).unwrap();
    let watcher = Watcher::with_refresh(cmd_rx, evt_tx, registry.clone(), provider.clone(), nav_tx);
    let handle = tokio::spawn(async move {
        watcher.run().await;
    });

    cmd_tx
        .send(location_watch(
            LocationRef::from_location(&Location::local(watched_path.clone())),
            SessionId(1),
        ))
        .unwrap();
    sleep(Duration::from_millis(20)).await;

    provider
        .emit(sibling_path.join("changed.txt"), FsChangeKind::Created)
        .await;
    sleep(Duration::from_millis(20)).await;

    assert!(
        collect_available_events(&evt_rx).is_empty(),
        "sibling paths should not produce FsChanged for the watched node"
    );
    assert!(
        nav_rx.try_recv().is_err(),
        "sibling paths should not invalidate the watched node"
    );

    drop(cmd_tx);
    let _ = timeout(Duration::from_secs(1), handle).await;
}
