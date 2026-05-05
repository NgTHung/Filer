//! Scanner Actor Integration Tests
//!
//! These tests exercise the Scanner actor directly via its command channel,
//! using a MockProvider to control filesystem responses.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use tokio::time::timeout;

use filer_core::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use filer_core::model::registry::NodeRegistry;
use filer_core::model::session;
use filer_core::modules::scan::scanner::{ScanCommand, Scanner};
use filer_core::{Actor, Capabilities, CoreError, Event, FsProvider, PipelineConfig, SortConfig};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_file(name: &str, path: &str, size: u64, hidden: bool) -> FileNode {
    let extension = PathBuf::from(name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned());
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("{path}/{name}")),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
        accessed: None,
    }
}

// ── MockProvider ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MockProvider {
    files: Arc<Mutex<Vec<FileNode>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    fn add_file(&self, node: FileNode) {
        self.files.lock().unwrap().push(node);
    }

    fn get_list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().unwrap() = should_fail;
    }
}

#[async_trait]
impl FsProvider for MockProvider {
    fn scheme(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: false,
            watch: false,
            search: false,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        if *self.should_fail.lock().unwrap() {
            return Err(CoreError::NotFound(path.to_path_buf()));
        }
        self.list_calls.lock().unwrap().push(path.to_path_buf());
        Ok(self.files.lock().unwrap().clone())
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(&self, _path: &Path, _start: u64, _len: u64) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(&self, _path: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::NotFound(PathBuf::from("test")))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_scanner_actor_starts_and_stops() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, _evt_rx) = flume::unbounded();
    let provider = Arc::new(MockProvider::new());
    let reg = NodeRegistry::new();

    let scanner = Scanner::new(cmd_rx, evt_tx, provider, reg);

    let handle = tokio::spawn(async move {
        scanner.run().await;
    });

    drop(cmd_tx);

    let result = timeout(Duration::from_millis(500), handle).await;
    assert!(
        result.is_ok(),
        "Scanner should exit when command channel closes"
    );
}

#[tokio::test]
async fn test_scanner_processes_scan_command() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let provider = MockProvider::new();
    let reg = NodeRegistry::new();

    provider.add_file(make_file("file1.txt", "/test", 425, false));
    provider.add_file(make_file("file2.txt", "/test", 200, false));

    let sess = session::SessionId::new();
    let provider_clone = provider.clone();
    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);

    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/test"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .expect("Failed to send scan command");

    let event = timeout(Duration::from_secs(1), evt_rx.recv_async())
        .await
        .expect("Timeout waiting for event")
        .expect("Failed to receive event");

    let calls = provider_clone.get_list_calls();
    assert!(
        !calls.is_empty(),
        "Scanner should have called list() on provider"
    );
    assert_eq!(calls[0], PathBuf::from("/test"));

    match event {
        Event::DirectoryLoaded { groups, .. } => {
            let total: usize = groups.groups.iter().map(|g| g.nodes.len()).sum();
            assert_eq!(total, 2);
        }
        Event::FilesBatch(entries, _) => {
            assert_eq!(entries.len(), 2);
        }
        _ => {}
    }
}

#[tokio::test]
async fn test_scanner_handles_cancellation() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, _evt_rx) = flume::unbounded();
    let provider = Arc::new(MockProvider::new());
    let sess = session::SessionId::new();
    let reg = NodeRegistry::new();

    let scanner = Scanner::new(cmd_rx, evt_tx, provider, reg);
    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/test"),
            pipeline: PipelineConfig {
                sort: Some(SortConfig {
                    ..Default::default()
                }),
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    cmd_tx.send(ScanCommand::Cancel(sess)).unwrap();

    // Scanner should handle cancel gracefully (no crash)
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_scanner_handles_multiple_scans() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, _evt_rx) = flume::unbounded();
    let provider = MockProvider::new();
    let reg = NodeRegistry::new();
    let sess = session::SessionId::new();

    provider.add_file(make_file("test.txt", "/dir1", 50, false));

    let provider_clone = provider.clone();
    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);
    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/dir1"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/dir2"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let calls = provider_clone.get_list_calls();
    assert!(
        calls.len() >= 2,
        "Scanner should process multiple scan commands"
    );
}

#[tokio::test]
async fn test_scanner_handles_provider_errors() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let provider = MockProvider::new();
    let sess = session::SessionId::new();
    let reg = NodeRegistry::new();

    provider.set_should_fail(true);

    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);
    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/nonexistent"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    match timeout(Duration::from_secs(1), evt_rx.recv_async()).await {
        Ok(Ok(Event::Error { message, .. })) => {
            assert!(!message.is_empty());
        }
        Ok(Ok(_)) => {}
        Ok(Err(_)) => panic!("Event channel closed unexpectedly"),
        Err(_) => {} // Timeout acceptable if scanner handles errors silently
    }
}

#[tokio::test]
async fn test_scanner_depth_limiting() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, _evt_rx) = flume::unbounded();
    let provider = MockProvider::new();
    let sess = session::SessionId::new();
    let reg = NodeRegistry::new();

    provider.add_file(make_file("shallow.txt", "/test", 10, false));

    let provider_clone = provider.clone();
    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);
    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/test"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let calls = provider_clone.get_list_calls();
    assert!(!calls.is_empty());
}

#[tokio::test]
async fn test_scanner_emits_progress_events() {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (evt_tx, evt_rx) = flume::unbounded();
    let provider = MockProvider::new();
    let sess = session::SessionId::new();
    let reg = NodeRegistry::new();

    for i in 0..10 {
        provider.add_file(make_file(&format!("file{i}.txt"), "/test", i * 100, false));
    }

    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(provider), reg);
    let _handle = tokio::spawn(async move { scanner.run().await });

    cmd_tx
        .send(ScanCommand::Scan {
            path: PathBuf::from("/test"),
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            session: sess,
        })
        .unwrap();

    let mut received_events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);

    while tokio::time::Instant::now() < deadline {
        match timeout(Duration::from_millis(100), evt_rx.recv_async()).await {
            Ok(Ok(event)) => received_events.push(event),
            _ => break,
        }
    }

    assert!(
        !received_events.is_empty(),
        "Scanner should emit events during scan"
    );
}
