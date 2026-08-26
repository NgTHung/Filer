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
use crate::errors::{CoreError, ErrorCode, ErrorTarget};
use crate::model::location::{Location, LocationRef};
use crate::model::node::{NodeEntry, NodeKind, NodeMeta};
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::search::searcher::{SearchCommand, SearchEventMode, Searcher};
use crate::tests::fixtures::{local_file_node, local_node_entry};
use crate::utils;
use crate::vfs::provider::{Capabilities, FsProvider};

const TIMEOUT: Duration = Duration::from_millis(3000);

/// Hierarchical mock filesystem for search testing.
/// Maps directory paths to their children, supporting recursive traversal.
/// Search tests use native entries throughout the provider boundary.
#[derive(Clone)]
struct MockProvider {
    files_by_path: Arc<Mutex<Vec<(PathBuf, Vec<NodeEntry>)>>>,
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
    delay_ms: Arc<Mutex<u64>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
            fail_paths: Arc::new(Mutex::new(Vec::new())),
            delay_ms: Arc::new(Mutex::new(0)),
        }
    }

    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<NodeEntry>) {
        self.files_by_path
            .lock()
            .unwrap()
            .push((dir.into(), children));
    }

    fn add_fail_path(&self, path: impl Into<PathBuf>) {
        self.fail_paths.lock().unwrap().push(path.into());
    }

    fn list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn set_delay_ms(&self, delay_ms: u64) {
        *self.delay_ms.lock().unwrap() = delay_ms;
    }

    fn make_file(name: &str, parent: &str, size: u64) -> NodeEntry {
        let extension = utils::get_extension(Path::new(name)).map(str::to_string);
        local_file_node(
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

    fn make_hidden_file(name: &str, parent: &str, size: u64) -> NodeEntry {
        let mut f = Self::make_file(name, parent, size);
        f.meta.hidden = true;
        f
    }

    fn make_dir(name: &str, parent: &str) -> NodeEntry {
        local_file_node(
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

    fn make_hidden_dir(name: &str, parent: &str) -> NodeEntry {
        let mut d = Self::make_dir(name, parent);
        d.meta.hidden = true;
        d
    }

    fn make_file_with_time(name: &str, parent: &str, size: u64, modified_secs: u64) -> NodeEntry {
        let mut f = Self::make_file(name, parent, size);
        f.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_secs));
        f
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
            search: true,
        }
    }

    async fn list(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        // Check if this path should fail
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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

    async fn exists(&self, _path: &Path, _cx: &crate::ProviderCx<'_>) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(
        &self,
        path: &Path,
        _cx: &crate::ProviderCx<'_>,
    ) -> Result<crate::NodeEntry, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

/// Collect all SearchResults batches until `complete: true`.
async fn wait_for_search_complete(
    evt_rx: &Receiver<Event>,
    expected_session: SessionId,
) -> Vec<crate::NodeEntry> {
    let mut matches = Vec::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::SearchResults {
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
            Ok(Ok(_)) => { /* skip non-search events */ }
            Ok(Err(_)) => panic!("event channel closed while waiting for SearchResults"),
            Err(_) => panic!("timed out waiting for SearchResults (complete: true)"),
        }
    }
}

/// Collect all SearchResults batches until `complete: true`.
async fn wait_for_search_entries_complete(
    evt_rx: &Receiver<Event>,
    expected_session: SessionId,
) -> Vec<crate::NodeEntry> {
    let mut matches = Vec::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(Event::SearchResults {
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
            Ok(Err(_)) => panic!("event channel closed while waiting for SearchResults"),
            Err(_) => panic!("timed out waiting for SearchResults (complete: true)"),
        }
    }
}

async fn wait_for_error(evt_rx: &Receiver<Event>, expected_session: SessionId) -> Event {
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
            Ok(Ok(event @ Event::Error { session, .. })) if session == expected_session => {
                return event;
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed while waiting for Error"),
            Err(_) => panic!("timed out waiting for Error event"),
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
    tokio::spawn(async move {
        searcher.run().await;
    });

    (cmd_tx, evt_rx)
}

fn spawn_searcher_with_timeout(
    provider: MockProvider,
    registry: NodeRegistry,
    search_timeout: Duration,
) -> (flume::Sender<SearchCommand>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = flume::unbounded::<SearchCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();

    let mut searcher = Searcher::new(cmd_rx, evt_tx, Arc::new(provider), registry);
    searcher.set_search_timeout(Some(search_timeout));
    tokio::spawn(async move {
        searcher.run().await;
    });

    (cmd_tx, evt_rx)
}

include!("searcher_timeout_tests.rs");

include!("searcher_location_tests.rs");

include!("searcher_lifecycle_tests.rs");

include!("searcher_basic_tests.rs");

include!("searcher_traversal_tests.rs");

include!("searcher_filter_tests.rs");

include!("searcher_hidden_tests.rs");

include!("searcher_limit_tests.rs");

include!("searcher_cancellation_tests.rs");

include!("searcher_error_tests.rs");

include!("searcher_session_tests.rs");
