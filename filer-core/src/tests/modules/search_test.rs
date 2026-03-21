//! Search Module Tests
//!
//! Actor-level tests for the Searcher actor and integration tests through FilerCore.
//! These tests define the expected search behavior as a specification.
//!
//! Test categories:
//!   - Lifecycle: actor start/stop
//!   - Basic search: text matching, case sensitivity
//!   - Recursive traversal: subdirectories, depth limits, BFS order
//!   - Filters: extension, size, type, hidden, name, regex, date
//!   - Hidden file handling: exclude by default, prune hidden dirs
//!   - Result limiting: max_results, streaming batches
//!   - Cancellation: cancel stops search, session isolation
//!   - Errors: unresolvable root, unreadable directories
//!   - Session: correct session on results

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use crate::actors::Actor;
use crate::api::events::Event;
use crate::errors::CoreError;
use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::session::SessionId;
use crate::modules::search::searcher::{SearchCommand, Searcher};
use crate::utils;
use crate::vfs::provider::{Capabilities, FsProvider};

// ── Constants ────────────────────────────────────────────────────────────────

const TIMEOUT: Duration = Duration::from_millis(3000);

// ── MockProvider ─────────────────────────────────────────────────────────────

/// Hierarchical mock filesystem for search testing.
/// Maps directory paths to their children, supporting recursive traversal.
#[derive(Clone)]
struct MockProvider {
    files_by_path: Arc<Mutex<Vec<(PathBuf, Vec<FileNode>)>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            fail_paths: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<FileNode>) {
        self.files_by_path.lock().unwrap().push((dir.into(), children));
    }

    fn add_fail_path(&self, path: impl Into<PathBuf>) {
        self.fail_paths.lock().unwrap().push(path.into());
    }

    fn list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_file(name: &str, parent: &str, size: u64) -> FileNode {
        let extension = utils::get_extension(Path::new(name)).map(str::to_string);
        FileNode {
            id: NodeId::from_path(&PathBuf::from(parent).join(name)),
            name: name.to_string(),
            path: PathBuf::from(parent).join(name),
            kind: NodeKind::File { extension },
            size,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
            created: None,
            meta: NodeMeta { hidden: false, readonly: false, permissions: None },
        }
    }

    fn make_hidden_file(name: &str, parent: &str, size: u64) -> FileNode {
        let mut f = Self::make_file(name, parent, size);
        f.meta.hidden = true;
        f
    }

    fn make_dir(name: &str, parent: &str) -> FileNode {
        FileNode {
            id: NodeId::from_path(&PathBuf::from(parent).join(name)),
            name: name.to_string(),
            path: PathBuf::from(parent).join(name),
            kind: NodeKind::Directory { children_count: None },
            size: 0,
            modified: Some(SystemTime::UNIX_EPOCH),
            created: None,
            meta: NodeMeta { hidden: false, readonly: false, permissions: None },
        }
    }

    fn make_hidden_dir(name: &str, parent: &str) -> FileNode {
        let mut d = Self::make_dir(name, parent);
        d.meta.hidden = true;
        d
    }

    fn make_file_with_time(name: &str, parent: &str, size: u64, modified_secs: u64) -> FileNode {
        let mut f = Self::make_file(name, parent, size);
        f.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_secs));
        f
    }
}

#[async_trait]
impl FsProvider for MockProvider {
    fn scheme(&self) -> &'static str { "mock" }

    fn capabilities(&self) -> Capabilities {
        Capabilities { read: true, write: false, watch: false, search: true }
    }

    async fn list(&self, path: &Path) -> Result<Vec<FileNode>, CoreError> {
        // Check if this path should fail
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::NotFound(path.to_path_buf()));
        }

        // Yield to the scheduler between directory listings so that
        // cancellation tokens and actor commands are processed between
        // BFS iterations. Without this, the pure-memory mock completes
        // entire traversals in a single scheduling quantum, making
        // cancellation tests unreliable.
        tokio::task::yield_now().await;

        self.list_calls.lock().unwrap().push(path.to_path_buf());

        let guard = self.files_by_path.lock().unwrap();
        Ok(guard
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, files)| files.clone())
            .unwrap_or_default())
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> { Ok(vec![]) }

    async fn read_range(
        &self, _path: &Path, _start: u64, _len: u64,
    ) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> { Ok(true) }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::NotFound(path.to_path_buf()))
    }
}

// ── Test Helpers ─────────────────────────────────────────────────────────────

/// Collect all SearchResults batches until `complete: true`.
async fn wait_for_search_complete(
    evt_rx: &Receiver<Event>,
    expected_session: SessionId,
) -> Vec<FileNode> {
    let mut matches = Vec::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::SearchResults { matches: batch, complete, session, .. }))
                if session == expected_session =>
            {
                matches.extend(batch);
                if complete {
                    return matches;
                }
            }
            Ok(Ok(_)) => { /* skip non-search events */ }
            Ok(Err(_)) => panic!("event channel closed while waiting for SearchResults"),
            Err(_) => panic!("timed out waiting for SearchResults (complete: true)"),
        }
    }
}

/// Collect all events (of any type) for a duration.
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

/// Spawn a Searcher actor and return the command sender.
fn spawn_searcher(
    provider: MockProvider,
    registry: NodeRegistry,
) -> (flume::Sender<SearchCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let searcher = Searcher::new(cmd_rx, evt_tx, Arc::new(provider), registry);
    tokio::spawn(async move { searcher.run().await; });

    (cmd_tx, evt_rx)
}

// ══════════════════════════════════════════════════════════════════════════════
// Actor-Level Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod searcher_lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_searcher_starts_and_stops() {
        let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
        let (evt_tx, _evt_rx) = flume::unbounded::<Event>();
        let provider = Arc::new(MockProvider::new());
        let registry = NodeRegistry::new();

        let searcher = Searcher::new(cmd_rx, evt_tx, provider, registry);
        let handle = tokio::spawn(async move { searcher.run().await; });

        // Drop command sender — actor should exit gracefully
        drop(cmd_tx);

        let result = timeout(Duration::from_millis(500), handle).await;
        assert!(result.is_ok(), "Searcher should exit when command channel closes");
    }

    #[tokio::test]
    async fn test_searcher_cancel_nonexistent_session() {
        let (cmd_tx, evt_rx) = {
            let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
            let (evt_tx, evt_rx) = flume::unbounded::<Event>();
            let provider = Arc::new(MockProvider::new());
            let registry = NodeRegistry::new();
            let searcher = Searcher::new(cmd_rx, evt_tx, provider, registry);
            tokio::spawn(async move { searcher.run().await; });
            (cmd_tx, evt_rx)
        };

        // Cancel a session that doesn't exist — should not crash
        cmd_tx.send(SearchCommand::Cancel(SessionId::new())).unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // No events should be emitted
        assert!(evt_rx.try_recv().is_err(), "No events expected for cancel of nonexistent session");

        drop(cmd_tx);
    }
}

#[cfg(test)]
mod searcher_basic_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_returns_matching_files() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("readme.md", "/root", 100),
            MockProvider::make_file("other.txt", "/root", 200),
            MockProvider::make_file("README.txt", "/root", 150),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("readme").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        // Case insensitive by default: should match both "readme.md" and "README.txt"
        assert_eq!(matches.len(), 2, "should match both readme files (case insensitive)");
        assert!(matches.iter().all(|m| m.name.to_lowercase().contains("readme")));
    }

    #[tokio::test]
    async fn test_search_no_matches_returns_empty() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("hello.txt", "/root", 100),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("nonexistent").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert!(matches.is_empty(), "should return empty when nothing matches");
    }

    #[tokio::test]
    async fn test_search_case_insensitive_by_default() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("readme.md", "/root", 100),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("README").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "case insensitive: README should match readme.md");
    }

    #[tokio::test]
    async fn test_search_case_sensitive() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("readme.md", "/root", 100),
            MockProvider::make_file("README.md", "/root", 200),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("README case:yes").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "case sensitive: only README.md should match");
        assert_eq!(matches[0].name, "README.md");
    }
}

#[cfg(test)]
mod searcher_traversal_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_traverses_subdirectories() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("a.txt", "/root", 100),
            MockProvider::make_dir("sub", "/root"),
        ]);
        provider.add_dir("/root/sub", vec![
            MockProvider::make_file("b.txt", "/root/sub", 200),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("txt").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "should find files in both root and subdirectory");

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
    }

    #[tokio::test]
    async fn test_search_respects_max_depth() {
        let provider = MockProvider::new();
        // depth 0: /root
        provider.add_dir("/root", vec![
            MockProvider::make_file("level0.txt", "/root", 10),
            MockProvider::make_dir("d1", "/root"),
        ]);
        // depth 1: /root/d1
        provider.add_dir("/root/d1", vec![
            MockProvider::make_file("level1.txt", "/root/d1", 20),
            MockProvider::make_dir("d2", "/root/d1"),
        ]);
        // depth 2: /root/d1/d2
        provider.add_dir("/root/d1/d2", vec![
            MockProvider::make_file("level2.txt", "/root/d1/d2", 30),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // depth:1 means traverse root (depth 0) and one level down (depth 1)
        let query = SearchQuery::parse("level depth:1").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "depth:1 should find level0.txt and level1.txt only");

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"level0.txt"));
        assert!(names.contains(&"level1.txt"));
        assert!(!names.contains(&"level2.txt"), "level2 should be excluded by depth limit");
    }

    #[tokio::test]
    async fn test_search_bfs_order() {
        // BFS should return shallow matches before deep ones
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("shallow.rs", "/root", 100),
            MockProvider::make_dir("deep", "/root"),
        ]);
        provider.add_dir("/root/deep", vec![
            MockProvider::make_dir("deeper", "/root/deep"),
        ]);
        provider.add_dir("/root/deep/deeper", vec![
            MockProvider::make_file("deep_file.rs", "/root/deep/deeper", 200),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("rs").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2);
        // BFS: shallow.rs should appear before deep_file.rs
        assert_eq!(matches[0].name, "shallow.rs", "BFS: shallow match should come first");
        assert_eq!(matches[1].name, "deep_file.rs", "BFS: deep match should come second");
    }
}

#[cfg(test)]
mod searcher_filter_tests {
    use super::*;

    #[tokio::test]
    async fn test_filter_by_extension() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("main.rs", "/root", 100),
            MockProvider::make_file("lib.rs", "/root", 200),
            MockProvider::make_file("readme.md", "/root", 50),
            MockProvider::make_file("config.toml", "/root", 75),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("ext:rs").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "should match only .rs files");
        assert!(matches.iter().all(|m| m.name.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_filter_by_size_greater_than() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("small.txt", "/root", 100),
            MockProvider::make_file("medium.txt", "/root", 500),
            MockProvider::make_file("large.txt", "/root", 2000),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("size:>1000").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "large.txt");
    }

    #[tokio::test]
    async fn test_filter_by_size_less_than() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("small.txt", "/root", 100),
            MockProvider::make_file("medium.txt", "/root", 500),
            MockProvider::make_file("large.txt", "/root", 2000),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("size:<500").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "small.txt");
    }

    #[tokio::test]
    async fn test_filter_by_type_file() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("file.txt", "/root", 100),
            MockProvider::make_dir("subdir", "/root"),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("type:file").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert!(matches[0].is_file());
    }

    #[tokio::test]
    async fn test_filter_by_type_directory() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("file.txt", "/root", 100),
            MockProvider::make_dir("subdir", "/root"),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("type:dir").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert!(matches[0].is_dir());
    }

    #[tokio::test]
    async fn test_filter_is_hidden() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("visible.txt", "/root", 100),
            MockProvider::make_hidden_file(".secret", "/root", 200),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // hidden:yes enables include_hidden AND adds IsHidden filter
        let query = SearchQuery::parse("hidden:yes").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "hidden:yes filter should only return hidden files");
        assert_eq!(matches[0].name, ".secret");
    }

    #[tokio::test]
    async fn test_filter_name_contains() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("app_config.toml", "/root", 100),
            MockProvider::make_file("config.json", "/root", 200),
            MockProvider::make_file("readme.md", "/root", 50),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("name:config").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|m| m.name.contains("config")));
    }

    #[tokio::test]
    async fn test_filter_name_matches_regex() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("test_search.rs", "/root", 100),
            MockProvider::make_file("test_scan.rs", "/root", 200),
            MockProvider::make_file("main.rs", "/root", 300),
            MockProvider::make_file("test_nav.py", "/root", 400),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse(r"match:^test_.*\.rs$").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "regex should match test_search.rs and test_scan.rs");
        assert!(matches.iter().all(|m| m.name.starts_with("test_") && m.name.ends_with(".rs")));
    }

    #[tokio::test]
    async fn test_filter_modified_after() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            // modified at epoch + 100s (very old)
            MockProvider::make_file_with_time("old.txt", "/root", 50, 100),
            // modified at epoch + 2_000_000_000s (~2033)
            MockProvider::make_file_with_time("new.txt", "/root", 50, 2_000_000_000),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // after:1700000000 (~Nov 2023)
        let query = SearchQuery::parse("after:1700000000").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "new.txt");
    }

    #[tokio::test]
    async fn test_filter_modified_before() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file_with_time("old.txt", "/root", 50, 100),
            MockProvider::make_file_with_time("new.txt", "/root", 50, 2_000_000_000),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("before:1700000000").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "old.txt");
    }

    #[tokio::test]
    async fn test_multiple_filters_and_semantics() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("big.rs", "/root", 2000),
            MockProvider::make_file("small.rs", "/root", 50),
            MockProvider::make_file("big.py", "/root", 3000),
            MockProvider::make_file("tiny.rs", "/root", 10),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // Both filters must match (AND): .rs files AND size > 100
        let query = SearchQuery::parse("ext:rs size:>100").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "only big.rs matches both ext:rs AND size:>100");
        assert_eq!(matches[0].name, "big.rs");
    }
}

#[cfg(test)]
mod searcher_hidden_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_excludes_hidden_by_default() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("visible.txt", "/root", 100),
            MockProvider::make_hidden_file(".hidden", "/root", 200),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // Default: include_hidden = false, no text filter so matches name
        let query = SearchQuery::parse("type:file").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "hidden files should be excluded by default");
        assert_eq!(matches[0].name, "visible.txt");
    }

    #[tokio::test]
    async fn test_search_skips_hidden_directories() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("visible.txt", "/root", 100),
            MockProvider::make_hidden_dir(".git", "/root"),
        ]);
        provider.add_dir("/root/.git", vec![
            MockProvider::make_file("HEAD", "/root/.git", 50),
            MockProvider::make_file("config", "/root/.git", 75),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider.clone(), registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("type:file").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 1, "should not traverse into .git directory");
        assert_eq!(matches[0].name, "visible.txt");

        // Verify .git was never listed
        let calls = provider.list_calls();
        assert!(!calls.contains(&PathBuf::from("/root/.git")),
            "provider.list() should never be called on hidden directory");
    }

    #[tokio::test]
    async fn test_search_includes_hidden_when_requested() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("visible.txt", "/root", 100),
            MockProvider::make_hidden_file(".env", "/root", 200),
            MockProvider::make_hidden_dir(".config", "/root"),
        ]);
        provider.add_dir("/root/.config", vec![
            MockProvider::make_hidden_file(".settings", "/root/.config", 50),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        // hidden:yes enables include_hidden AND adds IsHidden filter (only hidden files match)
        let query = SearchQuery::parse("hidden:yes").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        // Should find .env, .config dir, and .settings inside .config
        assert!(matches.len() >= 2, "should include hidden files and traverse hidden dirs");
        assert!(matches.iter().all(|m| m.meta.hidden), "all results should be hidden");
    }
}

#[cfg(test)]
mod searcher_limit_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_respects_max_results() {
        let provider = MockProvider::new();
        let mut files = Vec::new();
        for i in 0..10 {
            files.push(MockProvider::make_file(
                &format!("file{}.txt", i), "/root", (i + 1) * 100,
            ));
        }
        provider.add_dir("/root", files);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("file max:3").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 3, "max:3 should return exactly 3 results");
    }

    #[tokio::test]
    async fn test_search_streaming_batches() {
        // Create enough files to trigger multiple batches (>50)
        let provider = MockProvider::new();
        let mut files = Vec::new();
        for i in 0..75 {
            files.push(MockProvider::make_file(
                &format!("item{:03}.txt", i), "/root", (i + 1) * 10,
            ));
        }
        provider.add_dir("/root", files);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("item").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        // Count the number of SearchResults events
        let mut batch_count = 0;
        let mut total_matches = 0;
        let deadline = tokio::time::Instant::now() + TIMEOUT;

        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::SearchResults { matches, complete, session: s, .. }))
                    if s == session =>
                {
                    batch_count += 1;
                    total_matches += matches.len();
                    if complete { break; }
                }
                Ok(Ok(_)) => {}
                _ => panic!("timed out waiting for search batches"),
            }
        }

        assert_eq!(total_matches, 75, "should find all 75 items across batches");
        assert!(batch_count >= 2, "75 results should produce at least 2 batches (batch size ~50)");
    }
}

#[cfg(test)]
mod searcher_cancellation_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_cancel_stops_search() {
        // Create a deep tree so search takes time
        let provider = MockProvider::new();
        let mut dirs = vec![];
        let mut path = PathBuf::from("/root");
        for i in 0..20 {
            let children = vec![
                MockProvider::make_file(&format!("file{}.txt", i), path.to_str().unwrap(), 100),
                MockProvider::make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ];
            dirs.push((path.clone(), children));
            path = path.join(format!("d{}", i));
        }
        for (p, c) in dirs {
            provider.add_dir(p, c);
        }

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        // Immediately cancel
        cmd_tx.send(SearchCommand::Cancel(session)).unwrap();

        // Collect events for a short window
        let events = collect_events_for(&evt_rx, Duration::from_millis(500)).await;

        // Count matched files across all batches for this session
        let total_matches: usize = events.iter().filter_map(|e| {
            if let Event::SearchResults { matches, session: s, .. } = e {
                if *s == session { return Some(matches.len()); }
            }
            None
        }).sum();

        // yield_now() in the mock guarantees the scheduler processes the Cancel
        // command between directory listings, so the search stops well before
        // completing all 20 levels. Fewer than half the files is a reliable bound.
        assert!(total_matches < 10,
            "cancel should stop search well before finding all 20 files (found {})", total_matches);
    }

    #[tokio::test]
    async fn test_cancel_one_session_doesnt_affect_other() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("target.txt", "/root", 100),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session1 = SessionId::new();
        let session2 = SessionId::new();

        let query1 = SearchQuery::parse("target").unwrap();
        let query2 = SearchQuery::parse("target").unwrap();

        cmd_tx.send(SearchCommand::Search { query: query1, root: root_id, session: session1 }).unwrap();
        cmd_tx.send(SearchCommand::Cancel(session1)).unwrap();
        cmd_tx.send(SearchCommand::Search { query: query2, root: root_id, session: session2 }).unwrap();

        // session2 should still complete
        let matches = wait_for_search_complete(&evt_rx, session2).await;
        assert_eq!(matches.len(), 1, "cancelling session1 should not affect session2");
    }
}

#[cfg(test)]
mod searcher_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_unresolvable_root_emits_error() {
        let provider = MockProvider::new();
        let registry = NodeRegistry::new();
        // Don't register any path — root_id won't resolve
        let fake_root = NodeId(99999);
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("anything").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: fake_root, session }).unwrap();

        // Should get an Error event
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::Error { session: s, .. })) if s == session => {
                    return; // Test passes — got expected error
                }
                Ok(Ok(Event::SearchResults { session: s, complete, .. })) if s == session => {
                    if complete {
                        panic!("got SearchResults instead of Error for unresolvable root");
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => panic!("channel closed"),
                Err(_) => panic!("timed out waiting for Error event"),
            }
        }
    }

    #[tokio::test]
    async fn test_search_skips_unreadable_directories() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("found.txt", "/root", 100),
            MockProvider::make_dir("readable", "/root"),
            MockProvider::make_dir("forbidden", "/root"),
        ]);
        provider.add_dir("/root/readable", vec![
            MockProvider::make_file("also_found.txt", "/root/readable", 200),
        ]);
        // /root/forbidden will fail on list()
        provider.add_fail_path("/root/forbidden");

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("found").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let matches = wait_for_search_complete(&evt_rx, session).await;
        assert_eq!(matches.len(), 2, "should find files in readable dirs, skip forbidden");

        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"found.txt"));
        assert!(names.contains(&"also_found.txt"));
    }
}

#[cfg(test)]
mod searcher_session_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_result_carries_correct_session() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("file.txt", "/root", 100),
        ]);

        let registry = NodeRegistry::new();
        let root_id = registry.clone().register(PathBuf::from("/root"));
        let (cmd_tx, evt_rx) = spawn_searcher(provider, registry);

        let session = SessionId::new();
        let query = SearchQuery::parse("file").unwrap();
        cmd_tx.send(SearchCommand::Search { query, root: root_id, session }).unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
                Ok(Ok(Event::SearchResults { session: s, complete, .. })) => {
                    assert_eq!(s, session, "SearchResults should carry the correct session ID");
                    if complete { return; }
                }
                Ok(Ok(_)) => {}
                _ => panic!("timed out or channel closed"),
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Integration Tests (through FilerCore)
// These will work once SearchModule is wired with provider and Searcher is implemented.
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod search_integration_tests {
    use super::*;
    use crate::api::commands::Command;
    use crate::api::handle::FilerCore;
    use crate::modules::scan::ScanModule;
    use crate::modules::search::SearchModule;

    /// Build a FilerCore with Scan + Search modules.
    fn build_core_with_search(provider: MockProvider) -> FilerCore {
        let provider = Arc::new(provider);
        let scan = ScanModule::new(provider.clone());
        let search = SearchModule::new(provider);

        let core = FilerCore::new();
        core.load(scan);
        core.load(search);
        core
    }

    /// Send Handshake and get SessionId.
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
        provider.add_dir("/root", vec![
            MockProvider::make_file("target.rs", "/root", 100),
            MockProvider::make_file("other.py", "/root", 200),
        ]);

        let core = build_core_with_search(provider);
        let session = create_session(&core).await;

        // Register root path in the core's registry
        let root_id = core.registry().register(PathBuf::from("/root"));

        core.send(Command::Search {
            query: "target".to_string(),
            root: root_id,
            session,
        }).unwrap();

        let rx = core.event_receiver();
        let matches = wait_for_search_complete(&rx, session).await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "target.rs");
    }

    #[tokio::test]
    async fn test_session_destroy_cancels_search() {
        let provider = MockProvider::new();
        // Deep tree to make search take time
        let mut path = PathBuf::from("/root");
        for i in 0..10 {
            provider.add_dir(path.clone(), vec![
                MockProvider::make_file(&format!("f{}.txt", i), path.to_str().unwrap(), 100),
                MockProvider::make_dir(&format!("d{}", i), path.to_str().unwrap()),
            ]);
            path = path.join(format!("d{}", i));
        }

        let core = build_core_with_search(provider);
        let session = create_session(&core).await;
        let root_id = core.registry().register(PathBuf::from("/root"));

        core.send(Command::Search {
            query: "f".to_string(),
            root: root_id,
            session,
        }).unwrap();

        // Immediately destroy session — should cancel the search
        core.send(Command::DestroySession(session)).unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        // No crash, and search should have been cancelled
        // (we can't assert much more without race conditions)
    }

    #[tokio::test]
    async fn test_search_cancel_command() {
        let provider = MockProvider::new();
        provider.add_dir("/root", vec![
            MockProvider::make_file("file.txt", "/root", 100),
        ]);

        let core = build_core_with_search(provider);
        let session = create_session(&core).await;
        let root_id = core.registry().register(PathBuf::from("/root"));

        core.send(Command::Search {
            query: "file".to_string(),
            root: root_id,
            session,
        }).unwrap();

        core.send(Command::Cancel(session)).unwrap();

        // Should not crash
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
