//! Search Module Integration Tests
//!
//! These tests exercise the full command→event pipeline for search:
//!   FilerCore (Command::SearchNodeCompat) → Router → SearchModule → Searcher → Event::SearchResultsCompat
//!
//! The module stack used in every test:
//!   ScanModule::new(MockProvider) + SearchModule::new(MockProvider)

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use filer_core::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use filer_core::model::session::SessionId;
use filer_core::modules::scan::ScanModule;
use filer_core::modules::search::SearchModule;
use filer_core::{Capabilities, Command, CoreError, Event, FilerCore, FsProvider};

// ── Constants ─────────────────────────────────────────────────────────────────

const TIMEOUT: Duration = Duration::from_millis(3000);

// ── MockProvider ──────────────────────────────────────────────────────────────

/// Hierarchical in-memory filesystem for integration testing.
/// Maps directory paths to their children, supporting recursive traversal.
#[derive(Clone)]
struct MockProvider {
    files_by_path: Arc<Mutex<Vec<(PathBuf, Vec<FileNode>)>>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<FileNode>) {
        self.files_by_path
            .lock()
            .unwrap()
            .push((dir.into(), children));
    }
}

// ── Node helpers ──────────────────────────────────────────────────────────────

fn make_file(name: &str, parent: &str, size: u64) -> FileNode {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    FileNode {
        id: NodeId::from_path(&PathBuf::from(parent).join(name)),
        name: name.to_string(),
        path: PathBuf::from(parent).join(name),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
        accessed: None,
    }
}

fn make_dir(name: &str, parent: &str) -> FileNode {
    FileNode {
        id: NodeId::from_path(&PathBuf::from(parent).join(name)),
        name: name.to_string(),
        path: PathBuf::from(parent).join(name),
        kind: NodeKind::Directory {
            children_count: None,
        },
        size: 0,
        modified: Some(SystemTime::UNIX_EPOCH),
        created: None,
        meta: NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
        accessed: None,
    }
}

// ── FsProvider impl ───────────────────────────────────────────────────────────

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
            search: true,
        }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        // Yield to the scheduler between directory listings so that
        // cancellation tokens are processed between BFS iterations.
        tokio::task::yield_now().await;

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

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_core_with_search(provider: MockProvider) -> FilerCore {
    let provider = Arc::new(provider);
    let core = FilerCore::new();
    core.load(ScanModule::new(provider.clone()));
    core.load(SearchModule::new(provider));
    core
}

async fn create_session(core: &FilerCore) -> SessionId {
    let rx = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    match timeout(TIMEOUT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => id,
        other => panic!("expected SessionCreated, got {:?}", other),
    }
}

async fn wait_for_search_complete(
    evt_rx: &Receiver<Event>,
    expected_session: SessionId,
) -> Vec<FileNode> {
    let mut matches = Vec::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::SearchResultsCompat {
                matches: batch,
                complete,
                session,
                ..
            })) if session == expected_session => {
                matches.extend(batch);
                if complete {
                    return matches;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed while waiting for SearchResultsCompat"),
            Err(_) => panic!("timed out waiting for SearchResultsCompat (complete: true)"),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_command_through_filer_core() {
    let provider = MockProvider::new();
    provider.add_dir(
        "/root",
        vec![
            make_file("target.rs", "/root", 100),
            make_file("other.py", "/root", 200),
        ],
    );

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    let root_id = core.registry().register(PathBuf::from("/root"));

    core.send(Command::SearchNodeCompat {
        query: "target".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let rx = core.event_receiver();
    let matches = wait_for_search_complete(&rx, session).await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "target.rs");
}

#[tokio::test]
async fn test_session_destroy_cancels_search() {
    let provider = MockProvider::new();
    // Build a deep tree so search takes multiple scheduler quanta
    let mut path = PathBuf::from("/root");
    for i in 0..10 {
        provider.add_dir(
            path.clone(),
            vec![
                make_file(&format!("f{}.txt", i), path.to_str().unwrap(), 100),
                make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ],
        );
        path = path.join(format!("d{}", i));
    }

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    let root_id = core.registry().register(PathBuf::from("/root"));

    core.send(Command::SearchNodeCompat {
        query: "f".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    // Immediately destroy session — should trigger search cancellation
    core.send(Command::DestroySession(session)).unwrap();

    // No crash expected; search should have been cancelled
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[tokio::test]
async fn test_search_cancel_command() {
    let provider = MockProvider::new();
    provider.add_dir("/root", vec![make_file("file.txt", "/root", 100)]);

    let core = build_core_with_search(provider);
    let session = create_session(&core).await;
    let root_id = core.registry().register(PathBuf::from("/root"));

    core.send(Command::SearchNodeCompat {
        query: "file".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    core.send(Command::CancelSearch { session }).unwrap();

    // Should not crash
    tokio::time::sleep(Duration::from_millis(200)).await;
}
