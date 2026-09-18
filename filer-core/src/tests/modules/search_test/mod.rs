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

use crate::tests::fixtures::{nodes, provider::MemoryProvider};

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
use crate::model::node::NodeEntry;
use crate::model::query::SearchQuery;
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::search::searcher::{SearchCommand, SearchEventMode, Searcher};
use crate::vfs::provider::{Capabilities, FsProvider};

const TIMEOUT: Duration = Duration::from_millis(3000);

/// Delays and failed paths stay local to search timeout and cancellation tests.
#[derive(Clone)]
struct MockProvider {
    inner: MemoryProvider,
    fail_paths: Arc<Mutex<Vec<PathBuf>>>,
    delay_ms: Arc<Mutex<u64>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            inner: MemoryProvider::directories(true),
            fail_paths: Arc::new(Mutex::new(Vec::new())),
            delay_ms: Arc::new(Mutex::new(0)),
        }
    }

    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<NodeEntry>) {
        self.inner.add_dir(dir, children);
    }

    fn add_fail_path(&self, path: impl Into<PathBuf>) {
        self.fail_paths.lock().unwrap().push(path.into());
    }

    fn list_calls(&self) -> Vec<PathBuf> {
        self.inner.get_list_calls()
    }

    fn set_delay_ms(&self, delay_ms: u64) {
        *self.delay_ms.lock().unwrap() = delay_ms;
    }

    fn make_file(name: &str, parent: &str, size: u64) -> NodeEntry {
        nodes::file(name, parent, size)
    }

    fn make_hidden_file(name: &str, parent: &str, size: u64) -> NodeEntry {
        let mut f = Self::make_file(name, parent, size);
        f.meta.hidden = true;
        f
    }

    fn make_dir(name: &str, parent: &str) -> NodeEntry {
        nodes::directory(name, parent)
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
        self.inner.scheme()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    async fn list(
        &self,
        path: &Path,
        cx: &crate::ProviderCx<'_>,
    ) -> Result<Vec<crate::NodeEntry>, CoreError> {
        // Check if this path should fail
        if self.fail_paths.lock().unwrap().iter().any(|p| p == path) {
            return Err(CoreError::not_found(path.to_path_buf()));
        }

        let delay_ms = *self.delay_ms.lock().unwrap();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        self.inner.list(path, cx).await
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
    while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, evt_rx.recv_async()).await {
        events.push(event);
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
