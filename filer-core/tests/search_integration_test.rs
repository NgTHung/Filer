//! Search Module Integration Tests
//!
//! These tests exercise the full command→event pipeline for search:
//!   FilerCore (Command::Search) → Router → SearchModule → Searcher → Event::SearchResults
//!
//! The module stack used in every test:
//!   ScanModule::new(MockProvider) + SearchModule::new(MockProvider)

use support::state::SharedLog;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use tokio::time::timeout;

mod support;

use filer_core::model::node::{NodeEntry, NodeKind, NodeMeta};
use filer_core::model::session::SessionId;
use filer_core::modules::scan::ScanModule;
use filer_core::modules::search::SearchModule;
use filer_core::{Capabilities, Command, CoreError, Event, FilerCore, FsProvider};

use support::{local_location, make_entry, provider_entry, wait_for_search_entries};

const TIMEOUT: Duration = Duration::from_millis(3000);

/// Hierarchical in-memory filesystem for integration testing.
/// Maps directory paths to their children, supporting recursive traversal.
/// Search assertions use native entries throughout the provider boundary.
#[derive(Clone)]
struct MockProvider {
    files_by_path: SharedLog<(PathBuf, Vec<NodeEntry>)>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<NodeEntry>) {
        self.files_by_path
            .lock()
            .unwrap()
            .push((dir.into(), children));
    }
}

fn make_file(name: &str, parent: &str, size: u64) -> NodeEntry {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    make_entry(
        PathBuf::from(parent).join(name),
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
    make_entry(
        PathBuf::from(parent).join(name),
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

    async fn list(
        &self,
        path: &Path,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<Vec<filer_core::NodeEntry>, CoreError> {
        // Yield to the scheduler between directory listings so that
        // cancellation tokens are processed between BFS iterations.
        tokio::task::yield_now().await;

        let guard = self.files_by_path.lock().unwrap();
        Ok(guard
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, files)| files.iter().cloned().map(provider_entry).collect())
            .unwrap_or_default())
    }

    async fn read(
        &self,
        _path: &Path,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(
        &self,
        _path: &Path,
        _start: u64,
        _len: u64,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(
        &self,
        _path: &Path,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        path: &Path,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<filer_core::NodeEntry, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

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
    core.send(Command::Search {
        query: "target".to_string(),
        root: local_location("/root"),
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let rx = core.event_receiver();
    let matches = wait_for_search_entries(&rx, session, TIMEOUT).await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "target.rs");
    assert_eq!(matches[0].location, local_location("/root/target.rs"));
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
    core.send(Command::Search {
        query: "f".to_string(),
        root: local_location("/root"),
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
    core.send(Command::Search {
        query: "file".to_string(),
        root: local_location("/root"),
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    core.send(Command::CancelSearch { session }).unwrap();

    // Should not crash
    tokio::time::sleep(Duration::from_millis(200)).await;
}
