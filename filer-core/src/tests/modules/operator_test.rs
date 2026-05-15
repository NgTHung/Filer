//! Operator Module Tests
//!
//! Actor-level tests for the Operator actor.
//! These tests define the expected file-operation behavior as a specification.
//!
//! Test categories:
//!   - Lifecycle: actor start/stop
//!   - Copy: single file, recursive directory, multiple sources, errors, cancel
//!   - Move: same-filesystem (atomic rename), cross-filesystem (copy+delete)
//!   - Delete: permanent, trash (via injectable trash_fn), multiple targets, errors
//!   - Rename: success, directory, collision
//!   - Create: folder, file, collision
//!   - Cancel: mid-copy, session destroy

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use crate::actors::Actor;
use crate::api::events::{Event, OperationKind};
use crate::errors::CoreError;
use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::model::operation::OperationId;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::operations::operator::{Operator, OpsCommand};
use crate::vfs::provider::{Capabilities, FsProvider};

// ── Constants ────────────────────────────────────────────────────────────────

const TIMEOUT: Duration = Duration::from_millis(3000);

// ── MockOpsProvider ─────────────────────────────────────────────────────────

/// Mock filesystem provider for testing Operator behavior.
/// Tracks all write-method calls and supports configurable results.
#[derive(Clone)]
struct MockOpsProvider {
    /// Directory listings, keyed by path (for recursive copy)
    files_by_path: Arc<Mutex<Vec<(PathBuf, Vec<FileNode>)>>>,
    /// Paths that exist (for collision checks)
    existing_paths: Arc<Mutex<Vec<PathBuf>>>,
    /// Metadata results, keyed by path (for is-dir checks)
    metadata_results: Arc<Mutex<HashMap<PathBuf, FileNode>>>,
    /// Paths that should fail any operation
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
    /// If true, rename returns a cross-device error
    fail_rename_cross_device: Arc<Mutex<bool>>,

    // ── Call trackers ────────────────────────────────────────────────────
    copy_calls: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    rename_calls: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    delete_calls: Arc<Mutex<Vec<PathBuf>>>,
    mkdir_calls: Arc<Mutex<Vec<PathBuf>>>,
    write_calls: Arc<Mutex<Vec<(PathBuf, Vec<u8>)>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
}

impl MockOpsProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
            existing_paths: Arc::new(Mutex::new(Vec::new())),
            metadata_results: Arc::new(Mutex::new(HashMap::new())),
            fail_paths: Arc::new(Mutex::new(Vec::new())),
            fail_rename_cross_device: Arc::new(Mutex::new(false)),
            copy_calls: Arc::new(Mutex::new(Vec::new())),
            rename_calls: Arc::new(Mutex::new(Vec::new())),
            delete_calls: Arc::new(Mutex::new(Vec::new())),
            mkdir_calls: Arc::new(Mutex::new(Vec::new())),
            write_calls: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_existing(&self, path: impl Into<PathBuf>) {
        self.existing_paths.lock().unwrap().push(path.into());
    }

    fn add_metadata(&self, path: impl Into<PathBuf>, node: FileNode) {
        self.metadata_results
            .lock()
            .unwrap()
            .insert(path.into(), node);
    }

    fn add_dir_listing(&self, dir: impl Into<PathBuf>, children: Vec<FileNode>) {
        self.files_by_path
            .lock()
            .unwrap()
            .push((dir.into(), children));
    }

    fn add_fail_path(&self, path: impl Into<PathBuf>) {
        self.fail_paths.lock().unwrap().push(path.into());
    }

    fn set_cross_device(&self, val: bool) {
        *self.fail_rename_cross_device.lock().unwrap() = val;
    }

    fn get_copy_calls(&self) -> Vec<(PathBuf, PathBuf)> {
        self.copy_calls.lock().unwrap().clone()
    }

    fn get_rename_calls(&self) -> Vec<(PathBuf, PathBuf)> {
        self.rename_calls.lock().unwrap().clone()
    }

    fn get_delete_calls(&self) -> Vec<PathBuf> {
        self.delete_calls.lock().unwrap().clone()
    }

    fn get_mkdir_calls(&self) -> Vec<PathBuf> {
        self.mkdir_calls.lock().unwrap().clone()
    }

    fn get_write_calls(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.write_calls.lock().unwrap().clone()
    }

    // ── Node helpers ────────────────────────────────────────────────────

    fn make_file(name: &str, parent: &str, size: u64) -> FileNode {
        let path = PathBuf::from(parent).join(name);
        let extension = path.extension().map(|e| e.to_string_lossy().to_string());
        FileNode {
            id: NodeId::from_path(&path),
            name: name.to_string(),
            path,
            kind: NodeKind::File { extension },
            size,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
            created: None,
            accessed: None,
            meta: NodeMeta {
                hidden: false,
                readonly: false,
                permissions: None,
                ..Default::default()
            },
        }
    }

    fn make_dir(name: &str, parent: &str) -> FileNode {
        let path = PathBuf::from(parent).join(name);
        FileNode {
            id: NodeId::from_path(&path),
            name: name.to_string(),
            path,
            kind: NodeKind::Directory {
                children_count: None,
            },
            size: 0,
            modified: Some(SystemTime::UNIX_EPOCH),
            created: None,
            accessed: None,
            meta: NodeMeta {
                hidden: false,
                readonly: false,
                permissions: None,
                ..Default::default()
            },
        }
    }
}

#[async_trait]
impl FsProvider for MockOpsProvider {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }
        tokio::task::yield_now().await;
        self.list_calls.lock().unwrap().push(path.to_path_buf());
        let guard = self.files_by_path.lock().unwrap();
        Ok(guard
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, files)| files.clone())
            .unwrap_or_default())
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(&self, _path: &Path, _start: u64, _len: u64) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, path: &Path) -> Result<bool, CoreError> {
        Ok(self
            .existing_paths
            .lock()
            .unwrap()
            .iter()
            .any(|p| p == path))
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        self.metadata_results
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::not_found(path.to_path_buf()))
    }

    async fn write(&self, path: &Path, data: &[u8]) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        self.write_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), data.to_vec()));
        Ok(())
    }

    async fn copy(&self, src: &Path, dst: &Path) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == src) {
            return Err(CoreError::permission_denied(src.to_path_buf()));
        }
        tokio::task::yield_now().await;
        self.copy_calls
            .lock()
            .unwrap()
            .push((src.to_path_buf(), dst.to_path_buf()));
        Ok(())
    }

    async fn rename(&self, src: &Path, dst: &Path) -> Result<(), CoreError> {
        if *self.fail_rename_cross_device.lock().unwrap() {
            return Err(CoreError::io(
                src.to_path_buf(),
                "Invalid cross-device link (os error 18)",
            ));
        }
        if self.fail_paths.lock().unwrap().iter().any(|p| p == src) {
            return Err(CoreError::permission_denied(src.to_path_buf()));
        }
        self.rename_calls
            .lock()
            .unwrap()
            .push((src.to_path_buf(), dst.to_path_buf()));
        Ok(())
    }

    async fn delete(&self, path: &Path) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        self.delete_calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    async fn mkdir(&self, path: &Path) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        self.mkdir_calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

// ── Test Helpers ─────────────────────────────────────────────────────────────

/// A no-op trash function for tests that don't care about trash behavior.
fn noop_trash_fn() -> Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync> {
    Arc::new(|_path| Ok(()))
}

/// A trash function that records calls for assertion.
fn tracking_trash_fn() -> (
    Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>,
    Arc<Mutex<Vec<PathBuf>>>,
) {
    let calls: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let f: Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync> =
        Arc::new(move |path: &Path| {
            calls_clone.lock().unwrap().push(path.to_path_buf());
            Ok(())
        });
    (f, calls)
}

/// Spawn an Operator actor and return the command sender + event receiver.
fn spawn_operator(
    provider: MockOpsProvider,
    registry: NodeRegistry,
) -> (flume::Sender<OpsCommand>, Receiver<Event>) {
    spawn_operator_with_trash(provider, registry, noop_trash_fn())
}

/// Spawn an Operator actor with a custom trash function.
fn spawn_operator_with_trash(
    provider: MockOpsProvider,
    registry: NodeRegistry,
    trash_fn: Arc<dyn Fn(&Path) -> Result<(), CoreError> + Send + Sync>,
) -> (flume::Sender<OpsCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let operator = Operator::with_trash_fn(cmd_rx, evt_tx, Arc::new(provider), registry, trash_fn);
    tokio::spawn(async move {
        operator.run().await;
    });

    (cmd_tx, evt_rx)
}

/// Register a path in the registry and return its NodeId.
fn register(registry: &NodeRegistry, path: impl Into<PathBuf>) -> NodeId {
    registry.clone().register(path.into())
}

/// Collect all events until an OperationComplete or Error is received.
/// Returns (progress_events, final_event).
async fn wait_for_completion(
    evt_rx: &Receiver<Event>,
    expected_session: SessionId,
) -> (Vec<Event>, Event) {
    let mut progress_events = Vec::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(event @ Event::OperationComplete { session, .. }))
                if session == expected_session =>
            {
                return (progress_events, event);
            }
            Ok(Ok(event @ Event::Error { session, .. })) if session == expected_session => {
                return (progress_events, event);
            }
            Ok(Ok(event)) => {
                progress_events.push(event);
            }
            Ok(Err(_)) => panic!("event channel closed while waiting for OperationComplete"),
            Err(_) => panic!("timed out waiting for OperationComplete"),
        }
    }
}

/// Collect all events for a duration (used for cancel/no-event assertions).
async fn collect_events_for(evt_rx: &Receiver<Event>, duration: Duration) -> Vec<Event> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + duration;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(event)) => events.push(event),
            _ => break,
        }
    }
    events
}

fn assert_error_correlation(
    event: &Event,
    expected_session: SessionId,
    expected_request: RequestId,
    expected_operation: OperationId,
) {
    match event {
        Event::Error {
            session,
            request,
            operation,
            ..
        } => {
            assert_eq!(*session, expected_session);
            assert_eq!(*request, Some(expected_request));
            assert_eq!(*operation, Some(expected_operation));
        }
        other => panic!("Expected Error event, got: {other:?}"),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Lifecycle Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_operator_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let provider = Arc::new(MockOpsProvider::new());
        let registry = NodeRegistry::new();

        let operator = Operator::with_trash_fn(cmd_rx, evt_tx, provider, registry, noop_trash_fn());
        let handle = tokio::spawn(async move {
            operator.run().await;
        });

        drop(cmd_tx);

        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(
            result.is_ok(),
            "Operator should exit when command channel closes"
        );
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_session_does_not_crash() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let (cmd_tx, _evt_rx) = spawn_operator(provider, registry);

        let session = SessionId::new();
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !cmd_tx.is_disconnected(),
            "Operator should still be alive after cancelling unknown session"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Copy Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_single_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/backup");

        let src_id = register(&registry, &src_path);
        let dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/home/user", 1024),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert!(matches!(operation, OperationKind::Copy));
                assert!(success);
                assert!(!affected.is_empty());
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].0, src_path);
        assert_eq!(copies[0].1, dst_path.join("doc.txt"));
    }

    #[tokio::test]
    async fn test_copy_directory_recursive() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/project");
        let dst_dir = PathBuf::from("/home/user/backup");

        let src_id = register(&registry, &src_dir);
        let dst_id = register(&registry, &dst_dir);

        provider.add_metadata(&src_dir, MockOpsProvider::make_dir("project", "/home/user"));

        provider.add_dir_listing(
            &src_dir,
            vec![
                MockOpsProvider::make_file("a.txt", "/home/user/project", 100),
                MockOpsProvider::make_file("b.txt", "/home/user/project", 200),
                MockOpsProvider::make_file("c.txt", "/home/user/project", 300),
            ],
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: operation_id,
            })
            .unwrap();

        let (progress, final_event) = wait_for_completion(&evt_rx, session).await;

        // Should have OperationProgress events for each file copied
        let progress_count = progress
            .iter()
            .filter(|e| matches!(e, Event::OperationProgress { session: s, .. } if *s == session))
            .count();
        assert!(
            progress_count >= 3,
            "Recursive copy of 3 files should emit at least 3 progress events, got {progress_count}"
        );

        // Verify progress events have incrementing items_done
        let items_done_values: Vec<usize> = progress
            .iter()
            .filter_map(|e| match e {
                Event::OperationProgress {
                    operation_id: id,
                    items_done,
                    session: s,
                    ..
                } if *s == session && *id == operation_id => Some(*items_done),
                _ => None,
            })
            .collect();
        for window in items_done_values.windows(2) {
            assert!(
                window[1] >= window[0],
                "items_done should be non-decreasing: {items_done_values:?}"
            );
        }

        match final_event {
            Event::OperationComplete {
                operation,
                operation_id: id,
                success,
                ..
            } => {
                assert!(matches!(operation, OperationKind::Copy));
                assert_eq!(id, operation_id);
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        // All 3 files should have been copied
        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 3);

        // Destination directory should have been created
        let mkdirs = provider.get_mkdir_calls();
        assert!(!mkdirs.is_empty(), "Should create destination subdirectory");
    }

    #[tokio::test]
    async fn test_copy_multiple_sources() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src1 = PathBuf::from("/home/user/a.txt");
        let src2 = PathBuf::from("/home/user/b.txt");
        let dst = PathBuf::from("/home/user/backup");

        let src1_id = register(&registry, &src1);
        let src2_id = register(&registry, &src2);
        let dst_id = register(&registry, &dst);

        provider.add_metadata(
            &src1,
            MockOpsProvider::make_file("a.txt", "/home/user", 100),
        );
        provider.add_metadata(
            &src2,
            MockOpsProvider::make_file("b.txt", "/home/user", 200),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src1_id, src2_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                success, affected, ..
            } => {
                assert!(success);
                assert_eq!(affected.len(), 2);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 2);
    }

    #[tokio::test]
    async fn test_copy_error_emits_error_event() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/locked.txt");
        let dst_path = PathBuf::from("/home/user/backup");

        let src_id = register(&registry, &src_path);
        let dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("locked.txt", "/home/user", 100),
        );
        provider.add_fail_path(&src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::Error {
                recoverable,
                session: s,
                request,
                operation,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(request, Some(request_id));
                assert_eq!(operation, Some(operation_id));
                assert!(recoverable, "PermissionDenied should be recoverable");
            }
            other => panic!("Expected Error event, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_copy_unresolvable_source_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let fake_src = NodeId::from_path(&PathBuf::from("/nonexistent"));
        let dst_path = PathBuf::from("/home/user/backup");
        let dst_id = register(&registry, &dst_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![fake_src],
                destination: dst_id,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Move Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod move_tests {
    use super::*;

    #[tokio::test]
    async fn test_move_same_filesystem_atomic() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/doc.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let src_id = register(&registry, &src_path);
        let dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/home/user", 1024),
        );

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (progress_events, final_event) = wait_for_completion(&evt_rx, session).await;

        // Same-FS move: no progress events, just completion
        let progress_count = progress_events
            .iter()
            .filter(|e| matches!(e, Event::OperationProgress { .. }))
            .count();
        assert_eq!(
            progress_count, 0,
            "Same-filesystem move should not emit progress events"
        );

        match final_event {
            Event::OperationComplete {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Move));
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        // Should have used rename, not copy
        let renames = provider.get_rename_calls();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].0, src_path);
        assert_eq!(renames[0].1, dst_path.join("doc.txt"));

        assert!(
            provider.get_copy_calls().is_empty(),
            "Same-FS move should not copy"
        );
    }

    #[tokio::test]
    async fn test_move_cross_filesystem_fallback() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/mnt/usb/doc.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let src_id = register(&registry, &src_path);
        let dst_id = register(&registry, &dst_path);

        provider.add_metadata(
            &src_path,
            MockOpsProvider::make_file("doc.txt", "/mnt/usb", 1024),
        );
        provider.set_cross_device(true);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Move));
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        // Should have fallen back to copy + delete
        let copies = provider.get_copy_calls();
        assert_eq!(copies.len(), 1, "Cross-FS move should copy");

        let deletes = provider.get_delete_calls();
        assert_eq!(
            deletes.len(),
            1,
            "Cross-FS move should delete source after copy"
        );
        assert_eq!(deletes[0], src_path);
    }

    #[tokio::test]
    async fn test_move_error_emits_correlated_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/locked.txt");
        let dst_path = PathBuf::from("/home/user/archive");

        let src_id = register(&registry, &src_path);
        let dst_id = register(&registry, &dst_path);
        provider.add_fail_path(&src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Move {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Delete Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod delete_tests {
    use super::*;

    #[tokio::test]
    async fn test_delete_permanent() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/old.txt");
        let node_id = register(&registry, &path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: false,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert!(matches!(operation, OperationKind::Delete));
                assert!(success);
                assert_eq!(affected.len(), 1);
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let deletes = provider.get_delete_calls();
        assert_eq!(deletes.len(), 1);
        assert_eq!(deletes[0], path);
    }

    #[tokio::test]
    async fn test_delete_to_trash() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/old.txt");
        let node_id = register(&registry, &path);

        let (trash_fn, trash_calls) = tracking_trash_fn();
        let (cmd_tx, evt_rx) = spawn_operator_with_trash(provider.clone(), registry, trash_fn);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: true,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Delete));
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        // Should have called trash_fn, not provider.delete()
        let trashed = trash_calls.lock().unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0], path);

        assert!(
            provider.get_delete_calls().is_empty(),
            "trash:true should use trash_fn, not provider.delete()"
        );
    }

    #[tokio::test]
    async fn test_delete_multiple_targets() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let p1 = PathBuf::from("/home/user/a.txt");
        let p2 = PathBuf::from("/home/user/b.txt");
        let p3 = PathBuf::from("/home/user/c.txt");

        let id1 = register(&registry, &p1);
        let id2 = register(&registry, &p2);
        let id3 = register(&registry, &p3);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![id1, id2, id3],
                trash: false,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                success, affected, ..
            } => {
                assert!(success);
                assert_eq!(affected.len(), 3);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        assert_eq!(provider.get_delete_calls().len(), 3);
    }

    #[tokio::test]
    async fn test_delete_error_emits_error_event() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let path = PathBuf::from("/home/user/protected.txt");
        let node_id = register(&registry, &path);

        provider.add_fail_path(&path);

        let (cmd_tx, evt_rx) = spawn_operator(provider, registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Delete {
                targets: vec![node_id],
                trash: false,
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Rename Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod rename_tests {
    use super::*;

    #[tokio::test]
    async fn test_rename_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/old_name.txt");
        let src_id = register(&registry, &src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "new_name.txt".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Rename));
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let renames = provider.get_rename_calls();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].0, src_path);
        assert_eq!(
            renames[0].1,
            PathBuf::from("/home/user/new_name.txt"),
            "New path should be parent + new_name"
        );
    }

    #[tokio::test]
    async fn test_rename_directory() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/old_dir");
        let src_id = register(&registry, &src_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "new_dir".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation, success, ..
            } => {
                assert!(matches!(operation, OperationKind::Rename));
                assert!(success);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let renames = provider.get_rename_calls();
        assert_eq!(renames[0].1, PathBuf::from("/home/user/new_dir"));
    }

    #[tokio::test]
    async fn test_rename_collision_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_path = PathBuf::from("/home/user/file_a.txt");
        let src_id = register(&registry, &src_path);

        let collision_path = PathBuf::from("/home/user/file_b.txt");
        provider.add_existing(&collision_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::Rename {
                source: src_id,
                new_name: "file_b.txt".to_string(),
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);

        assert!(
            provider.get_rename_calls().is_empty(),
            "Should not rename when destination exists"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CreateFolder Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod create_folder_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_folder() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::CreateFolder {
                parent: parent_id,
                name: "new_folder".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert!(matches!(operation, OperationKind::CreateFolder));
                assert!(success);
                assert!(!affected.is_empty());
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let mkdirs = provider.get_mkdir_calls();
        assert_eq!(mkdirs.len(), 1);
        assert_eq!(mkdirs[0], PathBuf::from("/home/user/new_folder"));
    }

    #[tokio::test]
    async fn test_create_folder_collision_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        provider.add_existing(PathBuf::from("/home/user/existing_dir"));

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::CreateFolder {
                parent: parent_id,
                name: "existing_dir".to_string(),
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;
        assert_error_correlation(&final_event, session, request_id, operation_id);

        assert!(
            provider.get_mkdir_calls().is_empty(),
            "Should not create folder when it already exists"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CreateFile Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod create_file_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_file() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent: parent_id,
                name: "new_file.txt".to_string(),
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::OperationComplete {
                operation,
                success,
                affected,
                session: s,
                ..
            } => {
                assert!(matches!(operation, OperationKind::CreateFile));
                assert!(success);
                assert!(!affected.is_empty());
                assert_eq!(s, session);
            }
            other => panic!("Expected OperationComplete, got: {other:?}"),
        }

        let writes = provider.get_write_calls();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, PathBuf::from("/home/user/new_file.txt"));
        assert!(writes[0].1.is_empty(), "New file should be created empty");
    }

    #[tokio::test]
    async fn test_create_file_collision_emits_error() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let parent_path = PathBuf::from("/home/user");
        let parent_id = register(&registry, &parent_path);

        provider.add_existing(PathBuf::from("/home/user/exists.txt"));

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);
        let request_id = RequestId::new();
        let operation_id = OperationId::new();

        cmd_tx
            .send(OpsCommand::CreateFile {
                parent: parent_id,
                name: "exists.txt".to_string(),
                session,
                request: request_id,
                operation: operation_id,
            })
            .unwrap();

        let (_progress, final_event) = wait_for_completion(&evt_rx, session).await;

        match final_event {
            Event::Error {
                session: s,
                request,
                operation,
                ..
            } => {
                assert_eq!(s, session);
                assert_eq!(request, Some(request_id));
                assert_eq!(operation, Some(operation_id));
            }
            other => panic!("Expected Error for file collision, got: {other:?}"),
        }

        assert!(
            provider.get_write_calls().is_empty(),
            "Should not write when file already exists"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Cancel Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod cancel_tests {
    use super::*;

    #[tokio::test]
    async fn test_copy_cancel_midway() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/big_project");
        let dst_dir = PathBuf::from("/home/user/backup");

        let src_id = register(&registry, &src_dir);
        let dst_id = register(&registry, &dst_dir);

        provider.add_metadata(
            &src_dir,
            MockOpsProvider::make_dir("big_project", "/home/user"),
        );

        let mut files = Vec::new();
        for i in 0..50 {
            files.push(MockOpsProvider::make_file(
                &format!("file_{i}.txt"),
                "/home/user/big_project",
                100,
            ));
        }
        provider.add_dir_listing(&src_dir, files);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        // Give the operation a moment to start, then cancel
        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        // Collect events — should NOT get a successful OperationComplete
        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        let has_successful_complete = events.iter().any(|e| {
            matches!(
                e,
                Event::OperationComplete {
                    success: true,
                    session: s,
                    ..
                } if *s == session
            )
        });

        assert!(
            !has_successful_complete,
            "Cancelled copy should not emit OperationComplete with success"
        );

        // Should have copied fewer than 50 files
        let copies = provider.get_copy_calls();
        assert!(
            copies.len() < 50,
            "Cancel should stop before all files are copied (copied {})",
            copies.len()
        );
    }

    #[tokio::test]
    async fn test_session_destroy_cancels_operation() {
        let provider = MockOpsProvider::new();
        let registry = NodeRegistry::new();
        let session = SessionId::new();

        let src_dir = PathBuf::from("/home/user/big_project");
        let dst_dir = PathBuf::from("/home/user/backup");

        let src_id = register(&registry, &src_dir);
        let dst_id = register(&registry, &dst_dir);

        provider.add_metadata(
            &src_dir,
            MockOpsProvider::make_dir("big_project", "/home/user"),
        );

        let mut files = Vec::new();
        for i in 0..50 {
            files.push(MockOpsProvider::make_file(
                &format!("file_{i}.txt"),
                "/home/user/big_project",
                100,
            ));
        }
        provider.add_dir_listing(&src_dir, files);

        let (cmd_tx, evt_rx) = spawn_operator(provider.clone(), registry);

        cmd_tx
            .send(OpsCommand::Copy {
                sources: vec![src_id],
                destination: dst_id,
                session,
                request: RequestId::new(),
                operation: OperationId::new(),
            })
            .unwrap();

        tokio::task::yield_now().await;
        cmd_tx.send(OpsCommand::Cancel(session)).unwrap();

        let _events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        let copies = provider.get_copy_calls();
        assert!(
            copies.len() < 50,
            "Session destroy should cancel in-flight operation (copied {})",
            copies.len()
        );
    }
}
