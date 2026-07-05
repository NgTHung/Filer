//! Navigation Flow Integration Tests (Phase 4)
//!
//! These tests exercise the full command→event pipeline for navigation:
//!   FilerCore (Command) → Router → NavigationModule → Navigator → Scanner → Event
//!
//! The module stack used in every test:
//!   ScanModule::new(MockProvider) + NavigationModule::new(scan.sender())

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use filer_core::model::location::{LocationRef, LocationRoute};
use filer_core::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use filer_core::model::session::SessionId;
use filer_core::modules::navigation::NavigationModule;
use filer_core::modules::scan::ScanModule;
use filer_core::services::dir_cache::DirCache;
use filer_core::{Capabilities, Command, CoreError, Event, FilerCore, FsProvider, Location};

const TIMEOUT: Duration = Duration::from_millis(2000);

/// A simple in-memory filesystem provider for integration testing.
///
/// `files_by_path` maps a directory path to the `FileNode`s it contains.
/// By default, every path that isn't registered returns an empty listing so
/// that navigation to an unknown path doesn't produce an error.
#[derive(Clone)]
struct MockProvider {
    /// directory path → children
    files_by_path: Arc<Mutex<Vec<(PathBuf, Vec<FileNode>)>>>,
    /// Records every path that `list()` was called with
    list_calls: Arc<Mutex<Vec<PathBuf>>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            files_by_path: Arc::new(Mutex::new(Vec::new())),
            list_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register the children for a given directory path.
    fn add_dir(&self, dir: impl Into<PathBuf>, children: Vec<FileNode>) {
        self.files_by_path
            .lock()
            .unwrap()
            .push((dir.into(), children));
    }

    fn set_dir(&self, dir: impl Into<PathBuf>, children: Vec<FileNode>) {
        let dir = dir.into();
        let mut files = self.files_by_path.lock().unwrap();
        if let Some((_, existing)) = files.iter_mut().find(|(path, _)| *path == dir) {
            *existing = children;
        } else {
            files.push((dir, children));
        }
    }

    fn list_calls(&self) -> Vec<PathBuf> {
        self.list_calls.lock().unwrap().clone()
    }

    fn make_file(name: &str, parent: &str, size: u64) -> FileNode {
        FileNode {
            id: NodeId(name.len() as u64 ^ size),
            name: name.to_string(),
            path: PathBuf::from(parent).join(name),
            kind: NodeKind::File {
                extension: Path::new(name)
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned()),
            },
            size,
            modified: None,
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
        FileNode {
            id: NodeId(name.len() as u64 + 10_000),
            name: name.to_string(),
            path: PathBuf::from(parent).join(name),
            kind: NodeKind::Directory {
                children_count: None,
            },
            size: 0,
            modified: None,
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

    async fn list(
        &self,
        path: &Path,
        _cx: &filer_core::ProviderCx<'_>,
    ) -> Result<Vec<FileNode>, CoreError> {
        self.list_calls.lock().unwrap().push(path.to_path_buf());

        let guard = self.files_by_path.lock().unwrap();
        Ok(guard
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, files)| files.clone())
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
    ) -> Result<FileNode, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

/// Build a wired-up FilerCore with Navigation + Scan modules backed by `provider`.
fn build_core(provider: MockProvider) -> FilerCore {
    let cache = Arc::new(Mutex::new(DirCache::new(64 * 1024 * 1024)));
    let scan = ScanModule::with_cache(Arc::new(provider), cache);
    let nav = NavigationModule::new(scan.sender());

    let core = FilerCore::new();
    core.load(scan);
    core.load(nav);
    core
}

/// Send `Handshake` and return the `SessionId` from `SessionCreated`.
async fn create_session(core: &FilerCore) -> SessionId {
    let rx = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    match timeout(TIMEOUT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => id,
        other => panic!("expected SessionCreated, got {:?}", other),
    }
}

/// Wait until the event channel yields a directory load event for any session,
/// or panic on timeout.
async fn wait_for_directory_loaded(
    core: &FilerCore,
    expected_session: SessionId,
) -> (PathBuf, usize) {
    let rx = core.event_receiver();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            panic!("timed out waiting for directory load event");
        }
        match timeout(Duration::from_millis(100), rx.recv_async()).await {
            Ok(Ok(Event::DirectoryLoadedCompat {
                path,
                groups,
                session,
                ..
            })) => {
                if session == expected_session {
                    let total = groups.groups.iter().map(|g| g.nodes.len()).sum();
                    return (path, total);
                }
            }
            Ok(Ok(Event::DirectoryPageLoadedCompat {
                path,
                groups,
                session,
                ..
            })) => {
                if session == expected_session {
                    let total = groups.groups.iter().map(|g| g.nodes.len()).sum();
                    return (path, total);
                }
            }
            Ok(Ok(Event::DirectoryLoaded {
                parent,
                groups,
                session,
                ..
            })) => {
                if session == expected_session {
                    let path = path_from_location_ref(&parent)
                        .expect("test Location directory event should use direct path");
                    let total = groups.groups.iter().map(|g| g.nodes.len()).sum();
                    return (path, total);
                }
            }
            Ok(Ok(Event::DirectoryPageLoaded {
                parent,
                groups,
                session,
                ..
            })) => {
                if session == expected_session {
                    let path = path_from_location_ref(&parent)
                        .expect("test Location directory page event should use direct path");
                    let total = groups.groups.iter().map(|g| g.nodes.len()).sum();
                    return (path, total);
                }
            }
            Ok(Ok(Event::CurrentNavigateState { .. })) => { /* skip nav state snapshot */ }
            Ok(Ok(Event::Error {
                message, session, ..
            })) if session == expected_session => {
                panic!("got Error instead of directory load event: {}", message);
            }
            _ => {}
        }
    }
}

fn path_from_location_ref(location: &LocationRef) -> Option<PathBuf> {
    match location.descriptor()?.route() {
        LocationRoute::DirectPath { path } => Some(path),
        LocationRoute::Segmented { .. } | LocationRoute::UnsupportedProvider { .. } => None,
    }
}

#[cfg(test)]
mod navigation_flow_tests {
    use super::*;

    /// Navigate to a known directory → `DirectoryLoadedCompat` event is emitted
    /// with the correct session and path.
    #[tokio::test]
    async fn test_navigate_emits_directory_loaded() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/home/user/docs",
            vec![
                MockProvider::make_file("readme.md", "/home/user/docs", 512),
                MockProvider::make_file("notes.txt", "/home/user/docs", 128),
            ],
        );

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/home/user/docs"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let (path, count) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(
            path,
            PathBuf::from("/home/user/docs"),
            "path should match navigate target"
        );
        assert_eq!(count, 2, "should have received 2 files");
    }

    #[tokio::test]
    async fn test_location_navigate_emits_directory_loaded_and_state_location() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/location/docs",
            vec![MockProvider::make_file("readme.md", "/location/docs", 512)],
        );

        let core = build_core(provider);
        let session = create_session(&core).await;
        let location = Location::local("/location/docs");
        let location_ref = LocationRef::from_location(&location);
        let rx = core.event_receiver();

        core.send(Command::Navigate {
            location: location_ref,
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let mut loaded = false;
        let mut state_location = false;

        loop {
            assert!(
                tokio::time::Instant::now() <= deadline,
                "timeout waiting for LocationRef navigation events"
            );
            match timeout(Duration::from_millis(100), rx.recv_async()).await {
                Ok(Ok(Event::DirectoryPageLoaded {
                    parent,
                    groups,
                    session: s,
                    ..
                })) if s == session => {
                    assert_eq!(
                        path_from_location_ref(&parent),
                        Some(PathBuf::from("/location/docs"))
                    );
                    let count: usize = groups.groups.iter().map(|g| g.nodes.len()).sum();
                    assert_eq!(count, 1);
                    loaded = true;
                }
                Ok(Ok(Event::CurrentNavigateState { state, session: s })) if s == session => {
                    if state.current_location.as_ref().and_then(|r| r.descriptor())
                        == Some(location.descriptor())
                    {
                        state_location = true;
                    }
                }
                Ok(Ok(Event::Error {
                    message,
                    session: s,
                    ..
                })) if s == session => {
                    panic!("got Error instead of LocationRef navigation event: {message}");
                }
                _ => {}
            }

            if loaded && state_location {
                break;
            }
        }
    }

    /// Navigate to an empty directory should still emit `DirectoryLoadedCompat`
    /// (with 0 results — not an error).
    #[tokio::test]
    async fn test_navigate_to_empty_directory() {
        let provider = MockProvider::new();
        provider.add_dir("/empty", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/empty"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let (path, count) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(path, PathBuf::from("/empty"));
        assert_eq!(count, 0);
    }

    /// The `SessionId` on the `DirectoryLoadedCompat` event must match the session
    /// that issued the `Navigate` command.
    #[tokio::test]
    async fn test_navigate_event_carries_correct_session() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/data",
            vec![MockProvider::make_file("f.bin", "/data", 1024)],
        );

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/data"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let rx = core.event_receiver();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            assert!(tokio::time::Instant::now() <= deadline, "timeout");
            if let Ok(Ok(Event::DirectoryPageLoadedCompat {
                session: ev_session,
                ..
            })) = timeout(Duration::from_millis(100), rx.recv_async()).await
            {
                assert_eq!(
                    ev_session, session,
                    "event session must match command session"
                );
                return;
            }
        }
    }

    /// Navigate into a subdirectory, then `NavigateUp` → we land back in the parent
    /// and get a fresh `DirectoryLoadedCompat` for the parent path.
    #[tokio::test]
    async fn test_navigate_up_returns_to_parent() {
        let provider = MockProvider::new();
        provider.add_dir("/parent", vec![MockProvider::make_dir("child", "/parent")]);
        provider.add_dir(
            "/parent/child",
            vec![MockProvider::make_file("inner.txt", "/parent/child", 64)],
        );

        let core = build_core(provider);
        let session = create_session(&core).await;

        // Navigate into the child
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/parent/child"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (path, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(path, PathBuf::from("/parent/child"));

        // Navigate up — should land in /parent
        core.send(Command::NavigateUp {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (parent_path, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(
            parent_path,
            PathBuf::from("/parent"),
            "should have navigated to parent"
        );
    }

    /// `NavigateUp` preserves the session — subsequent commands work fine.
    #[tokio::test]
    async fn test_navigate_up_preserves_session() {
        let provider = MockProvider::new();
        provider.add_dir("/a/b", vec![]);
        provider.add_dir("/a", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;
        let rx = core.event_receiver();

        // Navigate to /a/b, go up, navigate further — no "Unknown session" error
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/a/b"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::NavigateUp {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        // Verify no error events were emitted for this session
        while let Ok(event) = rx.try_recv() {
            if let Event::Error {
                session: ev_session,
                message,
                ..
            } = &event
            {
                if ev_session == &session {
                    panic!("Unexpected error for session after NavigateUp: {}", message);
                }
            }
        }
    }

    /// `NavigateUp` from root (no parent) emits a recoverable `Error` rather
    /// than crashing or silently doing nothing.
    #[tokio::test]
    async fn test_navigate_up_from_root_emits_error() {
        let provider = MockProvider::new();
        provider.add_dir("/", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;

        // First navigate to root so the navigator has a current directory
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        // Now try to go up from root
        core.send(Command::NavigateUp {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;

        let rx = core.event_receiver();
        let events = {
            let mut v = Vec::new();
            while let Ok(e) = rx.try_recv() {
                v.push(e);
            }
            v
        };

        let found_error = events.iter().any(
            |e| matches!(e, Event::Error { session: s, recoverable: true, .. } if *s == session),
        );
        assert!(
            found_error,
            "NavigateUp from root should emit a recoverable Error"
        );
    }

    /// Refresh re-scans the current directory and emits `DirectoryLoadedCompat`.
    #[tokio::test]
    async fn test_refresh_emits_directory_loaded() {
        let provider = MockProvider::new();
        provider.add_dir("/docs", vec![MockProvider::make_file("a.txt", "/docs", 10)]);

        let core = build_core(provider.clone());
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/docs"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::Refresh {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (path, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(
            path,
            PathBuf::from("/docs"),
            "Refresh should reload the same directory"
        );
    }

    /// After a `Refresh`, the provider's `list` method is called a second time
    /// (the refresh actually hits the filesystem layer again).
    #[tokio::test]
    async fn test_refresh_rescans_provider() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/work",
            vec![MockProvider::make_file("todo.txt", "/work", 1)],
        );

        let core = build_core(provider.clone());
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/work"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        let calls_after_nav = provider.list_calls().len();

        core.send(Command::Refresh {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        let calls_after_refresh = provider.list_calls().len();
        assert!(
            calls_after_refresh > calls_after_nav,
            "Refresh should trigger an additional provider list() call",
        );
    }

    #[tokio::test]
    async fn test_refresh_bypasses_stale_directory_cache() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/fresh",
            vec![MockProvider::make_file("before.txt", "/fresh", 1)],
        );

        let core = build_core(provider.clone());
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/fresh"),
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (_, initial_count) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(initial_count, 1);

        provider.set_dir(
            "/fresh",
            vec![
                MockProvider::make_file("before.txt", "/fresh", 1),
                MockProvider::make_file("after.txt", "/fresh", 2),
            ],
        );

        core.send(Command::Refresh {
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (path, refreshed_count) = wait_for_directory_loaded(&core, session).await;

        assert_eq!(path, PathBuf::from("/fresh"));
        assert_eq!(
            refreshed_count, 2,
            "Refresh should emit the provider listing after cache invalidation"
        );
        assert_eq!(
            provider.list_calls().len(),
            2,
            "Refresh should bypass the stale cached listing"
        );
    }

    /// Refresh without an active navigation (no current dir) should emit a
    /// recoverable error, not crash.
    #[tokio::test]
    async fn test_refresh_without_current_dir_emits_error() {
        let provider = MockProvider::new();
        let core = build_core(provider);
        let session = create_session(&core).await;

        // No Navigate first — Refresh should fail gracefully
        core.send(Command::Refresh {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;

        let rx = core.event_receiver();
        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let found_error = events.iter().any(
            |e| matches!(e, Event::Error { session: s, recoverable: true, .. } if *s == session),
        );
        assert!(
            found_error,
            "Refresh with no current dir should emit a recoverable Error"
        );
    }

    /// Navigate A → B, then NavigateBack → we should get DirectoryLoadedCompat for A.
    #[tokio::test]
    async fn test_navigate_back_returns_to_previous() {
        let provider = MockProvider::new();
        provider.add_dir(
            "/dir_a",
            vec![MockProvider::make_file("x.rs", "/dir_a", 200)],
        );
        provider.add_dir(
            "/dir_b",
            vec![MockProvider::make_file("y.rs", "/dir_b", 300)],
        );

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/dir_a"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/dir_b"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::NavigateBack {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (path, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(
            path,
            PathBuf::from("/dir_a"),
            "Back should return to /dir_a"
        );
    }

    /// Navigate A → B → C, Navigate back twice → should land at A.
    #[tokio::test]
    async fn test_navigate_back_multiple_steps() {
        let provider = MockProvider::new();
        for dir in &["/a", "/b", "/c"] {
            provider.add_dir(*dir, vec![]);
        }

        let core = build_core(provider);
        let session = create_session(&core).await;

        for dir in &["/a", "/b", "/c"] {
            core.send(Command::NavigatePathCompat {
                path: PathBuf::from(dir),
                session: session,
                request: filer_core::RequestId::new(),
            })
            .unwrap();
            wait_for_directory_loaded(&core, session).await;
        }

        core.send(Command::NavigateBack {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (p, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(p, PathBuf::from("/b"), "first back should yield /b");

        core.send(Command::NavigateBack {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (p, _) = wait_for_directory_loaded(&core, session).await;
        assert_eq!(p, PathBuf::from("/a"), "second back should yield /a");
    }

    /// NavigateBack when there is no history should emit a recoverable `Error`.
    #[tokio::test]
    async fn test_navigate_back_with_no_history_emits_error() {
        let provider = MockProvider::new();
        provider.add_dir("/only", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;

        // Single navigation — no history to go back to
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/only"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::NavigateBack {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;

        let rx = core.event_receiver();
        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let found_error = events.iter().any(
            |e| matches!(e, Event::Error { session: s, recoverable: true, .. } if *s == session),
        );
        assert!(
            found_error,
            "Back with no history should emit a recoverable Error"
        );
    }

    /// Navigate A → B → back to A → the NavState snapshot should report `can_forward = true`.
    #[tokio::test]
    async fn test_navigate_back_then_forward() {
        let provider = MockProvider::new();
        provider.add_dir("/alpha", vec![]);
        provider.add_dir("/beta", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/alpha"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/beta"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        // Go back — immediately start listening for the NavState snapshot.
        let rx = core.event_receiver();
        core.send(Command::NavigateBack {
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let mut can_forward = false;
        let mut dir_loaded = false;

        loop {
            assert!(
                tokio::time::Instant::now() <= deadline,
                "timeout waiting for back events"
            );
            match timeout(Duration::from_millis(100), rx.recv_async()).await {
                Ok(Ok(Event::DirectoryPageLoadedCompat {
                    path, session: s, ..
                })) if s == session => {
                    assert_eq!(path, PathBuf::from("/alpha"), "back should land on /alpha");
                    dir_loaded = true;
                }
                Ok(Ok(Event::DirectoryPageLoaded {
                    parent, session: s, ..
                })) if s == session => {
                    assert_eq!(
                        path_from_location_ref(&parent),
                        Some(PathBuf::from("/alpha")),
                        "back should land on /alpha"
                    );
                    dir_loaded = true;
                }
                Ok(Ok(Event::CurrentNavigateState { state, session: s })) if s == session => {
                    if state.can_forward {
                        can_forward = true;
                    }
                }
                _ => {}
            }
            if can_forward && dir_loaded {
                break;
            }
        }

        assert!(
            can_forward,
            "after navigating back, can_forward should be true in NavState"
        );
    }

    /// Two independent sessions navigating different paths must each receive
    /// their own `DirectoryLoadedCompat` events with the correct session tag.
    #[tokio::test]
    async fn test_two_sessions_navigate_independently() {
        let provider = MockProvider::new();
        provider.add_dir("/s1", vec![MockProvider::make_file("a.txt", "/s1", 1)]);
        provider.add_dir(
            "/s2",
            vec![
                MockProvider::make_file("b.txt", "/s2", 2),
                MockProvider::make_file("c.txt", "/s2", 3),
            ],
        );

        let core = build_core(provider);
        let rx = core.event_receiver();

        core.send(Command::Handshake).unwrap();
        let s1 = match timeout(TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("{:?}", other),
        };
        core.send(Command::Handshake).unwrap();
        let s2 = match timeout(TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("{:?}", other),
        };

        assert_ne!(s1, s2);

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/s1"),
            session: s1,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/s2"),
            session: s2,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let (p1, c1) = wait_for_directory_loaded(&core, s1).await;
        let (p2, c2) = wait_for_directory_loaded(&core, s2).await;

        assert_eq!(p1, PathBuf::from("/s1"));
        assert_eq!(c1, 1);
        assert_eq!(p2, PathBuf::from("/s2"));
        assert_eq!(c2, 2);
    }

    /// Destroying one session must not affect the other session's navigation.
    #[tokio::test]
    async fn test_destroy_one_session_does_not_break_other() {
        let provider = MockProvider::new();
        provider.add_dir("/stay", vec![]);
        provider.add_dir("/gone", vec![]);

        let core = build_core(provider);
        let rx = core.event_receiver();

        core.send(Command::Handshake).unwrap();
        let session_stay = match timeout(TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("{:?}", other),
        };
        core.send(Command::Handshake).unwrap();
        let session_gone = match timeout(TIMEOUT, rx.recv_async()).await {
            Ok(Ok(Event::SessionCreated(id))) => id,
            other => panic!("{:?}", other),
        };

        // Navigate both sessions
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/stay"),
            session: session_stay,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/gone"),
            session: session_gone,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session_stay).await;
        wait_for_directory_loaded(&core, session_gone).await;

        // Destroy one
        core.send(Command::DestroySession(session_gone)).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The surviving session should still navigate without errors
        core.send(Command::Refresh {
            session: session_stay,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        let (path, _) = wait_for_directory_loaded(&core, session_stay).await;
        assert_eq!(
            path,
            PathBuf::from("/stay"),
            "surviving session should still work"
        );
    }

    /// After Navigate, the `CurrentNavigateState` event must carry a snapshot
    /// with `current` set and `can_back = false` (first navigation).
    #[tokio::test]
    async fn test_navigate_emits_nav_state_snapshot() {
        let provider = MockProvider::new();
        provider.add_dir("/snap", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;
        let rx = core.event_receiver();

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/snap"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        // Collect events for a moment
        tokio::time::sleep(Duration::from_millis(500)).await;
        let mut events: Vec<Event> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }

        let nav_state_event = events
            .iter()
            .find(|e| matches!(e, Event::CurrentNavigateState { session: s, .. } if *s == session));
        assert!(
            nav_state_event.is_some(),
            "should have received CurrentNavigateState"
        );

        if let Some(Event::CurrentNavigateState { state, .. }) = nav_state_event {
            assert!(
                state.current.is_some(),
                "current should be set after Navigate"
            );
            assert!(
                !state.can_back,
                "can_back should be false on first navigate"
            );
        }
    }

    /// After navigating A → B, the snapshot should report `can_back = true`.
    #[tokio::test]
    async fn test_nav_state_can_back_after_second_navigate() {
        let provider = MockProvider::new();
        provider.add_dir("/x", vec![]);
        provider.add_dir("/y", vec![]);

        let core = build_core(provider);
        let session = create_session(&core).await;

        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/x"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        wait_for_directory_loaded(&core, session).await;

        let rx = core.event_receiver();
        core.send(Command::NavigatePathCompat {
            path: PathBuf::from("/y"),
            session: session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let can_back;

        loop {
            assert!(
                tokio::time::Instant::now() <= deadline,
                "timeout waiting for can_back"
            );
            match timeout(Duration::from_millis(100), rx.recv_async()).await {
                Ok(Ok(Event::CurrentNavigateState { state, session: s })) if s == session => {
                    if state.can_back {
                        can_back = true;
                        break;
                    }
                }
                Ok(Ok(Event::DirectoryPageLoadedCompat { session: s, .. })) if s == session => {}
                _ => {}
            }
        }

        assert!(
            can_back,
            "can_back should be true after navigating to a second directory"
        );
    }
}
