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

use crate::tests::fixtures::state::SharedLog;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::errors::{CoreError, ErrorCode, ErrorContext, ErrorTarget};
use crate::model::capability::LocationCapabilityError;
use crate::model::location::{
    Location, LocationDescriptor, LocationId, LocationRef, LocationSegment, ProviderRef,
};
use crate::model::node::{NodeEntry, NodeKind, NodeMeta};
use crate::model::operation::{OperationId, OperationKind};
use crate::model::progress::{ProgressKind, ProgressStatus, ProgressTarget};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::operations::operator::{OperationEventMode, Operator, OpsCommand, TrashFn};
use crate::services::dir_cache::{DirCache, SharedDirCache};
use crate::tests::fixtures::{local_file_node, local_node_entry};
use crate::vfs::provider::{Capabilities, FsProvider, ListingOptions};

const TIMEOUT: Duration = Duration::from_millis(3000);

/// Mock filesystem provider for testing Operator behavior.
/// Tracks all write-method calls and supports configurable results.
/// Operation assertions use native entries, events, and targets.
#[derive(Clone)]
struct MockOpsProvider {
    /// Directory listings, keyed by path (for recursive copy)
    files_by_path: SharedLog<(PathBuf, Vec<NodeEntry>)>,
    /// Paths that exist (for collision checks)
    existing_paths: Arc<Mutex<Vec<PathBuf>>>,
    /// Metadata results, keyed by path (for is-dir checks)
    metadata_results: Arc<Mutex<HashMap<PathBuf, NodeEntry>>>,
    /// Paths that should fail any operation
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
    /// If true, rename returns a cross-device error
    fail_rename_cross_device: Arc<Mutex<bool>>,
    write_supported: Arc<Mutex<bool>>,
    /// Artificial delay applied inside `copy`, for deterministic timeout tests.
    copy_delay_ms: Arc<Mutex<u64>>,
    rename_delay_ms: Arc<Mutex<u64>>,
    delete_delay_ms: Arc<Mutex<u64>>,

    copy_calls: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    rename_calls: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    delete_calls: Arc<Mutex<Vec<PathBuf>>>,
    mkdir_calls: Arc<Mutex<Vec<PathBuf>>>,
    write_calls: SharedLog<(PathBuf, Vec<u8>)>,
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
            write_supported: Arc::new(Mutex::new(true)),
            copy_delay_ms: Arc::new(Mutex::new(0)),
            rename_delay_ms: Arc::new(Mutex::new(0)),
            delete_delay_ms: Arc::new(Mutex::new(0)),
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

    fn add_metadata(&self, path: impl Into<PathBuf>, node: NodeEntry) {
        self.metadata_results
            .lock()
            .unwrap()
            .insert(path.into(), node);
    }

    fn add_dir_listing(&self, dir: impl Into<PathBuf>, children: Vec<NodeEntry>) {
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

    fn set_write_supported(&self, supported: bool) {
        *self.write_supported.lock().unwrap() = supported;
    }

    fn set_copy_delay_ms(&self, delay_ms: u64) {
        *self.copy_delay_ms.lock().unwrap() = delay_ms;
    }

    fn set_rename_delay_ms(&self, delay_ms: u64) {
        *self.rename_delay_ms.lock().unwrap() = delay_ms;
    }

    fn set_delete_delay_ms(&self, delay_ms: u64) {
        *self.delete_delay_ms.lock().unwrap() = delay_ms;
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

    fn make_file(name: &str, parent: &str, size: u64) -> NodeEntry {
        let path = PathBuf::from(parent).join(name);
        let extension = path.extension().map(|e| e.to_string_lossy().to_string());
        local_file_node(
            path,
            name,
            NodeKind::File { extension },
            size,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
            NodeMeta {
                hidden: false,
                readonly: false,
                permissions: None,
                ..Default::default()
            },
        )
    }

    fn make_dir(name: &str, parent: &str) -> NodeEntry {
        let path = PathBuf::from(parent).join(name);
        local_file_node(
            path,
            name,
            NodeKind::Directory {
                children_count: None,
            },
            0,
            Some(SystemTime::UNIX_EPOCH),
            NodeMeta {
                hidden: false,
                readonly: false,
                permissions: None,
                ..Default::default()
            },
        )
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
            write: *self.write_supported.lock().unwrap(),
            watch: false,
            search: false,
        }
    }

    async fn list(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }
        tokio::task::yield_now().await;
        self.list_calls.lock().unwrap().push(path.to_path_buf());
        let guard = self.files_by_path.lock().unwrap();
        Ok(guard
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, files)| files.iter().cloned().map(local_node_entry).collect())
            .unwrap_or_default())
    }

    async fn read(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(self
            .existing_paths
            .lock()
            .unwrap()
            .iter()
            .any(|p| p == path))
    }

    async fn metadata(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::NodeEntry, CoreError> {
        let node = self
            .metadata_results
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| CoreError::not_found(path.to_path_buf()))?;
        Ok(local_node_entry(node))
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        self.write_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), data.to_vec()));
        Ok(())
    }

    async fn copy(
        &self,
        src: &Path,
        dst: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == src) {
            return Err(CoreError::permission_denied(src.to_path_buf()));
        }
        let delay_ms = *self.copy_delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        } else {
            tokio::task::yield_now().await;
        }
        self.copy_calls
            .lock()
            .unwrap()
            .push((src.to_path_buf(), dst.to_path_buf()));
        Ok(())
    }

    async fn rename(
        &self,
        src: &Path,
        dst: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<(), CoreError> {
        if *self.fail_rename_cross_device.lock().unwrap() {
            return Err(CoreError::io(
                src.to_path_buf(),
                "Invalid cross-device link (os error 18)",
            ));
        }
        if self.fail_paths.lock().unwrap().iter().any(|p| p == src) {
            return Err(CoreError::permission_denied(src.to_path_buf()));
        }
        let delay_ms = *self.rename_delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        } else {
            tokio::task::yield_now().await;
        }
        self.rename_calls
            .lock()
            .unwrap()
            .push((src.to_path_buf(), dst.to_path_buf()));
        Ok(())
    }

    async fn delete(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        let delay_ms = *self.delete_delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        } else {
            tokio::task::yield_now().await;
        }
        self.delete_calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    async fn mkdir(&self, path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<(), CoreError> {
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::permission_denied(path.to_path_buf()));
        }
        self.mkdir_calls.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// A no-op trash function for tests that don't care about trash behavior.
fn noop_trash_fn() -> TrashFn {
    Arc::new(|_path| Ok(()))
}

/// A trash function that records calls for assertion.
fn tracking_trash_fn() -> (
    TrashFn,
    Arc<Mutex<Vec<PathBuf>>>,
) {
    let calls: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let f: TrashFn =
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
    trash_fn: TrashFn,
) -> (flume::Sender<OpsCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let operator = Operator::with_trash_fn(cmd_rx, evt_tx, Arc::new(provider), registry, trash_fn);
    tokio::spawn(async move {
        operator.run().await;
    });

    (cmd_tx, evt_rx)
}

fn spawn_operator_with_timeout(
    provider: MockOpsProvider,
    registry: NodeRegistry,
    timeout: Duration,
) -> (flume::Sender<OpsCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let mut operator = Operator::with_trash_fn(
        cmd_rx,
        evt_tx,
        Arc::new(provider),
        registry,
        noop_trash_fn(),
    );
    operator.set_operation_timeout(Some(timeout));
    tokio::spawn(async move {
        operator.run().await;
    });

    (cmd_tx, evt_rx)
}

fn spawn_operator_with_cache(
    provider: MockOpsProvider,
    registry: NodeRegistry,
    cache: SharedDirCache,
) -> (flume::Sender<OpsCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<OpsCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let operator = Operator::with_cache(cmd_rx, evt_tx, Arc::new(provider), registry, cache);
    tokio::spawn(async move {
        operator.run().await;
    });

    (cmd_tx, evt_rx)
}

fn local_ref(path: impl Into<PathBuf>) -> LocationRef {
    LocationRef::from_location(&Location::local(path.into()))
}

/// Collect all events until a Location-native OperationComplete or Error is received.
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
    while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
        events.push(event);
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

fn error_context(event: &Event) -> Option<&ErrorContext> {
    match event {
        Event::Error { context, .. } => context.as_deref(),
        other => panic!("Expected Error event, got: {other:?}"),
    }
}

include!("lifecycle_tests.rs");

include!("copy_tests.rs");

include!("move_tests.rs");

include!("delete_tests.rs");

include!("rename_tests.rs");

include!("create_folder_tests.rs");

include!("create_file_tests.rs");

include!("location_operation_tests.rs");

include!("cache_invalidation_tests.rs");

include!("cancel_tests.rs");

include!("operator_timeout_tests.rs");
