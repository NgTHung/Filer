//! Stress tests for filer-core.
//!
//! These tests exercise the system under large data volumes to verify
//! correctness and absence of panics/deadlocks, not raw throughput.
//!
//! All tests use an in-memory mock provider (no real I/O), so even large
//! datasets complete in milliseconds.
//!
//! Run all (including ignored):
//!   cargo test -p filer-core --test stress_test -- --include-ignored --nocapture
//!
//! Run a single test:
//!   cargo test -p filer-core --test stress_test stress_search_large_flat_dir -- --nocapture

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use flume::Receiver;
use tokio::time::timeout;

use filer_core::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use filer_core::model::registry::NodeRegistry;
use filer_core::model::session::SessionId;
use filer_core::modules::scan::ScanModule;
use filer_core::modules::scan::scanner::{ScanCommand, Scanner};
use filer_core::modules::search::SearchModule;
use filer_core::{
    Actor, Capabilities, Command, CoreError, Event, FilerCore, FsProvider, PipelineConfig,
};

const SHORT: Duration = Duration::from_secs(5);
const LONG: Duration = Duration::from_secs(30);

/// High-throughput in-memory provider for stress testing.
/// Uses a HashMap for O(1) directory lookups — essential when traversing
/// tens of thousands of directories.
#[derive(Clone)]
struct MockFs {
    dirs: Arc<Mutex<HashMap<PathBuf, Vec<FileNode>>>>,
}

impl MockFs {
    fn new() -> Self {
        Self {
            dirs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add_dir(&self, path: impl Into<PathBuf>, children: Vec<FileNode>) {
        self.dirs.lock().unwrap().insert(path.into(), children);
    }

    fn file_count(&self) -> usize {
        self.dirs
            .lock()
            .unwrap()
            .values()
            .flat_map(|v| v.iter().filter(|n| n.is_file()))
            .count()
    }
}

#[async_trait]
impl FsProvider for MockFs {
    fn scheme(&self) -> &'static str {
        "stress-mock"
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
        // Yield between listings so cancellation tokens are processed
        tokio::task::yield_now().await;
        let guard = self.dirs.lock().unwrap();
        Ok(guard.get(path).cloned().unwrap_or_default())
    }

    async fn read(&self, _path: &Path) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn read_range(&self, _path: &Path, _s: u64, _l: u64) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }

    async fn exists(&self, _path: &Path) -> Result<bool, CoreError> {
        Ok(true)
    }

    async fn metadata(&self, path: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::not_found(path.to_path_buf()))
    }
}

//
// Instead of pre-generating and storing every FileNode, this provider
// computes directory children on-the-fly from the path depth alone.
// Memory cost: O(width) per `list()` call — the result Vec — nothing stored.
// This makes arbitrarily large trees testable without OOM.

struct LazyTreeFs {
    root: PathBuf,
    width: usize,     // subdirectory fan-out per level
    max_depth: usize, // 0 = root only (no subdirs); root is depth 0
    files_per_dir: usize,
    match_every: usize, // file at index f matches when f % match_every == 0
}

impl LazyTreeFs {
    fn new(
        root: impl Into<PathBuf>,
        width: usize,
        max_depth: usize,
        files_per_dir: usize,
        match_every: usize,
    ) -> Self {
        Self {
            root: root.into(),
            width,
            max_depth,
            files_per_dir,
            match_every,
        }
    }

    /// Total directories in the tree (sum of geometric series).
    fn total_dirs(&self) -> usize {
        let mut total = 0usize;
        let mut n = 1usize;
        for _ in 0..=self.max_depth {
            total = total.saturating_add(n);
            n = n.saturating_mul(self.width);
        }
        total
    }

    /// Total files that will match the "match_" query.
    fn expected_matches(&self) -> usize {
        let per_dir = (self.files_per_dir + self.match_every - 1) / self.match_every;
        self.total_dirs() * per_dir
    }

    fn depth_of(&self, path: &Path) -> Option<usize> {
        path.strip_prefix(&self.root)
            .ok()
            .map(|r| r.components().count())
    }

    fn children_of(&self, path: &Path) -> Vec<FileNode> {
        let d = match self.depth_of(path) {
            Some(d) => d,
            None => return vec![],
        };
        let mut out = Vec::with_capacity(self.files_per_dir + self.width);
        for f in 0..self.files_per_dir {
            let name = if f % self.match_every == 0 {
                format!("match_{:04}.rs", f)
            } else {
                format!("file_{:04}.log", f)
            };
            out.push(file(&name, path, (f as u64 + 1) * 256));
        }
        if d < self.max_depth {
            for w in 0..self.width {
                out.push(dir(&format!("w{}", w), path));
            }
        }
        out
    }
}

#[async_trait]
impl FsProvider for LazyTreeFs {
    fn scheme(&self) -> &'static str {
        "lazy-tree"
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
        tokio::task::yield_now().await;
        Ok(self.children_of(path))
    }

    async fn read(&self, _p: &Path) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }
    async fn read_range(&self, _p: &Path, _s: u64, _l: u64) -> Result<Vec<u8>, CoreError> {
        Ok(vec![])
    }
    async fn exists(&self, _p: &Path) -> Result<bool, CoreError> {
        Ok(true)
    }
    async fn metadata(&self, p: &Path) -> Result<FileNode, CoreError> {
        Err(CoreError::not_found(p.to_path_buf()))
    }
}

fn file(name: &str, parent: &Path, size: u64) -> FileNode {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_string);
    FileNode {
        id: NodeId::from_path(&parent.join(name)),
        name: name.to_string(),
        path: parent.join(name),
        kind: NodeKind::File { extension: ext },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size % 1_000_000)),
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

fn dir(name: &str, parent: &Path) -> FileNode {
    FileNode {
        id: NodeId::from_path(&parent.join(name)),
        name: name.to_string(),
        path: parent.join(name),
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

fn build_core(fs: MockFs) -> FilerCore {
    let fs = Arc::new(fs);
    let core = FilerCore::new();
    core.load(ScanModule::new(fs.clone()));
    core.load(SearchModule::new(fs));
    core
}

async fn handshake(core: &FilerCore) -> (SessionId, Receiver<Event>) {
    let rx = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    match timeout(SHORT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => (id, rx),
        other => panic!("expected SessionCreated, got {:?}", other),
    }
}

/// Create a session using an already-open receiver.
/// Use this (instead of `handshake`) when multiple sessions share one `FilerCore`,
/// to avoid competing receivers stealing each other's events on the flume channel.
async fn create_session(core: &FilerCore, rx: &Receiver<Event>) -> SessionId {
    core.send(Command::Handshake).unwrap();
    match timeout(SHORT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => id,
        other => panic!("expected SessionCreated, got {:?}", other),
    }
}

/// Drain SearchResultsCompat until `complete: true`, returning all matched files.
async fn drain_search(
    rx: &Receiver<Event>,
    session: SessionId,
    deadline: Duration,
) -> Vec<FileNode> {
    let mut all = Vec::new();
    let limit = tokio::time::Instant::now() + deadline;
    loop {
        match tokio::time::timeout_at(limit, rx.recv_async()).await {
            Ok(Ok(Event::SearchResultsCompat {
                matches,
                complete,
                session: s,
                ..
            })) if s == session => {
                all.extend(matches);
                if complete {
                    return all;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed before search completed"),
            Err(_) => panic!(
                "search timed out after {:.1}s — found {} matches so far",
                deadline.as_secs_f32(),
                all.len()
            ),
        }
    }
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_search_large_flat_dir() {
    const TOTAL: usize = 5_000;
    const MATCHES: usize = TOTAL / 2;

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/flat");

    let children: Vec<FileNode> = (0..TOTAL)
        .map(|i| {
            let name = if i % 2 == 0 {
                format!("target_{:04}.rs", i) // even → matches "target"
            } else {
                format!("other_{:04}.rs", i)
            };
            file(&name, &root, (i as u64 + 1) * 512)
        })
        .collect();

    fs.add_dir(root.clone(), children);
    assert_eq!(fs.file_count(), TOTAL);

    let core = build_core(fs);
    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(root);

    core.send(Command::SearchNodeCompat {
        query: "target".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let results = drain_search(&rx, session, LONG).await;
    assert_eq!(
        results.len(),
        MATCHES,
        "expected exactly {} matches in a flat dir of {} files",
        MATCHES,
        TOTAL
    );
    assert!(
        results.iter().all(|n| n.name.contains("target")),
        "every result must contain 'target' in its name"
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_search_deep_tree() {
    const DEPTH: usize = 300;

    let fs = MockFs::new();
    let mut path = PathBuf::from("/stress/deep");

    for i in 0..DEPTH {
        let subdir_name = format!("level_{:03}", i);
        let children = vec![
            file(&format!("needle_{:03}.txt", i), &path, 1024),
            dir(&subdir_name, &path),
        ];
        fs.add_dir(path.clone(), children);
        path = path.join(subdir_name);
    }
    // Leaf directory — one final file, no further subdirs
    fs.add_dir(
        path.clone(),
        vec![file(&format!("needle_{:03}.txt", DEPTH), &path, 1024)],
    );

    let core = build_core(fs);
    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(PathBuf::from("/stress/deep"));

    core.send(Command::SearchNodeCompat {
        query: "needle".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let results = drain_search(&rx, session, LONG).await;
    assert_eq!(
        results.len(),
        DEPTH + 1,
        "BFS must visit all {} levels and find a needle at each",
        DEPTH + 1
    );
}

//
// Uses LazyTreeFs so no FileNodes are stored up front.
// Memory during traversal: O(width^depth) PathBufs in the BFS queue at peak
//   = 10^4 = 10 000 PathBufs ≈ 640 KB. Results ≈ 66 666 FileNodes ≈ 20 MB.

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_search_wide_deep_tree() {
    // width=10, max_depth=5 → 1+10+100+1000+10000+100000 = 111 111 dirs
    // 20 files/dir, every 5th matches → 4 matches/dir → 444 444 total matches
    let root = PathBuf::from("/stress/wide");
    let lazy = LazyTreeFs::new(root.clone(), 10, 5, 20, 5);
    let expected = lazy.expected_matches();

    let fs = Arc::new(lazy);
    let core = FilerCore::new();
    core.load(ScanModule::new(fs.clone()));
    core.load(SearchModule::new(fs));

    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(root);

    core.send(Command::SearchNodeCompat {
        query: "match".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let results = drain_search(&rx, session, LONG).await;
    assert_eq!(
        results.len(),
        expected,
        "wide+deep lazy tree: expected {} matches",
        expected
    );
    assert!(
        results.iter().all(|n| n.name.starts_with("match_")),
        "all results must start with 'match_'"
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_concurrent_sessions() {
    const SESSIONS: usize = 8;
    const FILES_PER_DIR: usize = 200;
    const EXPECTED_MATCHES: usize = 100; // every other file matches "hit"

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/concurrent");
    let children: Vec<FileNode> = (0..FILES_PER_DIR)
        .map(|i| {
            let name = if i % 2 == 0 {
                format!("hit_{:03}.txt", i)
            } else {
                format!("miss_{:03}.txt", i)
            };
            file(&name, &root, 1024)
        })
        .collect();
    fs.add_dir(root.clone(), children);

    let core = build_core(fs);
    let root_id = core.registry().register(root);

    // ONE receiver for the whole core — multiple receivers would race on the
    // flume MPMC channel and steal each other's events.
    let rx = core.event_receiver();

    // Create sessions sequentially so each SessionCreated arrives in order
    let mut sessions = Vec::new();
    for _ in 0..SESSIONS {
        sessions.push(create_session(&core, &rx).await);
    }

    // Fire off all searches at once
    for &session in &sessions {
        core.send(Command::SearchNodeCompat {
            query: "hit".to_string(),
            root: root_id,
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
    }

    // Drain from the shared receiver, routing results by session ID
    let mut counts: std::collections::HashMap<SessionId, usize> =
        sessions.iter().map(|&s| (s, 0)).collect();
    let mut completed = 0usize;
    let limit = tokio::time::Instant::now() + LONG;

    while completed < SESSIONS {
        match tokio::time::timeout_at(limit, rx.recv_async()).await {
            Ok(Ok(Event::SearchResultsCompat {
                matches,
                complete,
                session,
                ..
            })) => {
                if let Some(c) = counts.get_mut(&session) {
                    *c += matches.len();
                    if complete {
                        completed += 1;
                    }
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("channel closed after {}/{} sessions", completed, SESSIONS),
            Err(_) => panic!(
                "timed out after {}/{} sessions completed",
                completed, SESSIONS
            ),
        }
    }

    for (i, &session) in sessions.iter().enumerate() {
        assert_eq!(
            counts[&session], EXPECTED_MATCHES,
            "session {} got {} matches, expected {}",
            i, counts[&session], EXPECTED_MATCHES
        );
    }
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_rapid_cancel_restart() {
    const CYCLES: usize = 30;

    // Deep tree so each search actually has work to cancel
    let fs = MockFs::new();
    let mut path = PathBuf::from("/stress/cancel");
    for i in 0..50 {
        let sub = format!("sub_{}", i);
        fs.add_dir(
            path.clone(),
            vec![file(&format!("f{}.txt", i), &path, 1024), dir(&sub, &path)],
        );
        path = path.join(sub);
    }
    fs.add_dir(
        path,
        vec![file("last.txt", Path::new("/stress/cancel/leaf"), 512)],
    );

    let core = build_core(fs);
    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(PathBuf::from("/stress/cancel"));

    // Rapid fire: search → cancel, repeated CYCLES times
    for _ in 0..CYCLES {
        core.send(Command::SearchNodeCompat {
            query: "f".to_string(),
            root: root_id,
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();
        core.send(Command::CancelSearch { session }).unwrap();
    }

    // Drain any partial results from cancelled searches
    tokio::time::sleep(Duration::from_millis(200)).await;
    while rx.try_recv().is_ok() {}

    // Final search must complete correctly without hangs or panics
    core.send(Command::SearchNodeCompat {
        query: "f".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    let results = drain_search(&rx, session, LONG).await;
    assert!(
        !results.is_empty(),
        "final search after {} cancel cycles must still return results",
        CYCLES
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_all_filters_combined() {
    // Query: ext:rs size:>1000 after:1000000000
    // Only files that satisfy ALL THREE criteria match.
    let fs = MockFs::new();
    let root = PathBuf::from("/stress/filters");

    const TOTAL: usize = 2_000;
    let base_ts = 1_000_000_000u64; // Sep 2001 in Unix seconds

    let mut expected = 0usize;
    let children: Vec<FileNode> = (0..TOTAL)
        .map(|i| {
            let is_rs = i % 3 == 0;
            let is_big = i % 4 == 0; // size > 1000
            let is_new = i % 5 == 0; // modified after base_ts

            let ext = if is_rs { "rs" } else { "py" };
            let size = if is_big { 2048u64 } else { 100u64 };
            let ts = if is_new { base_ts + 1 } else { base_ts - 1 };

            if is_rs && is_big && is_new {
                expected += 1;
            }

            let name = format!("file_{:04}.{}", i, ext);
            let mut node = file(&name, &root, size);
            node.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(ts));
            node
        })
        .collect();

    fs.add_dir(root.clone(), children);

    let core = build_core(fs);
    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(root);

    // ext:rs AND size:>1000 AND after:<base_ts>
    let query = format!("ext:rs size:>1000 after:{}", base_ts);
    core.send(Command::SearchNodeCompat {
        query,
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    let results = drain_search(&rx, session, LONG).await;
    assert_eq!(
        results.len(),
        expected,
        "combined filters: expected {} files matching ext:rs + size:>1000 + after:ts",
        expected
    );
    assert!(
        results.iter().all(|n| n.name.ends_with(".rs")),
        "all results must be .rs files"
    );
    assert!(
        results.iter().all(|n| n.size > 1000),
        "all results must be > 1000 bytes"
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_streaming_batch_accuracy() {
    // 750 files: all match. Default batch size is 50 → at least 15 batches.
    const TOTAL: usize = 750;

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/batches");
    let children: Vec<FileNode> = (0..TOTAL)
        .map(|i| file(&format!("item_{:04}.txt", i), &root, (i as u64 + 1) * 10))
        .collect();
    fs.add_dir(root.clone(), children);

    let core = build_core(fs);
    let (session, rx) = handshake(&core).await;
    let root_id = core.registry().register(root);

    core.send(Command::SearchNodeCompat {
        query: "item".to_string(),
        root: root_id,
        session,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    // Manually count batches and total items to verify no results are dropped
    let mut total = 0usize;
    let mut batch_count = 0usize;
    let limit = tokio::time::Instant::now() + LONG;

    loop {
        match tokio::time::timeout_at(limit, rx.recv_async()).await {
            Ok(Ok(Event::SearchResultsCompat {
                matches,
                complete,
                session: s,
                ..
            })) if s == session => {
                assert!(
                    !matches.is_empty() || complete,
                    "every non-final batch must be non-empty"
                );
                total += matches.len();
                batch_count += 1;
                if complete {
                    break;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("channel closed"),
            Err(_) => panic!(
                "batch stream timed out after {} items in {} batches",
                total, batch_count
            ),
        }
    }

    assert_eq!(
        total, TOTAL,
        "all {} items must arrive across {} batches",
        TOTAL, batch_count
    );
    assert!(
        batch_count >= 2,
        "750 items with batch_size=50 must produce >= 2 batches, got {}",
        batch_count
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_scanner_large_dir() {
    const FILE_COUNT: usize = 3_000;

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/scan");
    let children: Vec<FileNode> = (0..FILE_COUNT)
        .map(|i| file(&format!("entry_{:05}.dat", i), &root, (i as u64 + 1) * 64))
        .collect();
    fs.add_dir(root.clone(), children);

    let (cmd_tx, cmd_rx) = flume::unbounded::<ScanCommand>();
    let (evt_tx, evt_rx) = flume::unbounded::<Event>();
    let reg = NodeRegistry::new();

    let scanner = Scanner::new(cmd_rx, evt_tx, Arc::new(fs), reg);
    tokio::spawn(async move { scanner.run().await });

    let session = SessionId::new();
    cmd_tx
        .send(ScanCommand::Scan {
            path: root,
            pipeline: PipelineConfig {
                sort: None,
                filter: None,
                group: None,
            },
            load: filer_core::DirectoryLoadOptions::default(),
            session,
            request: filer_core::RequestId::new(),
        })
        .unwrap();

    let event = timeout(LONG, evt_rx.recv_async())
        .await
        .expect("scanner timed out")
        .expect("event channel closed");

    let total = match event {
        Event::DirectoryLoadedCompat { groups, .. } => {
            groups.groups.iter().map(|g| g.nodes.len()).sum::<usize>()
        }
        Event::FilesBatch(nodes, _) => nodes.len(),
        other => panic!("unexpected event: {:?}", other),
    };

    assert_eq!(
        total, FILE_COUNT,
        "scanner must emit all {} files in the directory",
        FILE_COUNT
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_session_isolation_under_cancellation() {
    const FILES: usize = 500;

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/isolation");
    let children: Vec<FileNode> = (0..FILES)
        .map(|i| file(&format!("doc_{:04}.rs", i), &root, 1024))
        .collect();
    fs.add_dir(root.clone(), children);

    let core = build_core(fs);
    let root_id = core.registry().register(root);

    // ONE receiver shared across all sessions
    let rx = core.event_receiver();
    let sa = create_session(&core, &rx).await;
    let sb = create_session(&core, &rx).await;
    let sc = create_session(&core, &rx).await;
    let sd = create_session(&core, &rx).await;

    // Launch A and B, then immediately cancel them
    core.send(Command::SearchNodeCompat {
        query: "doc".to_string(),
        root: root_id,
        session: sa,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    core.send(Command::SearchNodeCompat {
        query: "doc".to_string(),
        root: root_id,
        session: sb,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    core.send(Command::CancelSearch { session: sa }).unwrap();
    core.send(Command::CancelSearch { session: sb }).unwrap();

    // Launch C and D — these must complete despite A/B cancellations
    core.send(Command::SearchNodeCompat {
        query: "doc".to_string(),
        root: root_id,
        session: sc,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    core.send(Command::SearchNodeCompat {
        query: "doc".to_string(),
        root: root_id,
        session: sd,
        request: filer_core::RequestId::new(),
    })
    .unwrap();

    // Drain the shared receiver, counting results only for C and D
    let mut count_c = 0usize;
    let mut count_d = 0usize;
    let mut done_c = false;
    let mut done_d = false;
    let limit = tokio::time::Instant::now() + LONG;

    while !done_c || !done_d {
        match tokio::time::timeout_at(limit, rx.recv_async()).await {
            Ok(Ok(Event::SearchResultsCompat {
                matches,
                complete,
                session,
                ..
            })) => {
                if session == sc {
                    count_c += matches.len();
                    if complete {
                        done_c = true;
                    }
                } else if session == sd {
                    count_d += matches.len();
                    if complete {
                        done_d = true;
                    }
                }
                // sa/sb events are intentionally discarded
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("channel closed before C and D completed"),
            Err(_) => panic!("timed out — C done={}, D done={}", done_c, done_d),
        }
    }

    assert_eq!(
        count_c, FILES,
        "session C must find all {} files despite A/B cancellations",
        FILES
    );
    assert_eq!(
        count_d, FILES,
        "session D must find all {} files despite A/B cancellations",
        FILES
    );
}

#[tokio::test]
#[ignore = "stress test — run with: cargo test --test stress_test -- --include-ignored"]
async fn stress_search_determinism() {
    const FILES: usize = 1_000;

    let fs = MockFs::new();
    let root = PathBuf::from("/stress/determinism");
    let children: Vec<FileNode> = (0..FILES)
        .map(|i| file(&format!("stable_{:04}.rs", i), &root, (i as u64 + 1) * 100))
        .collect();
    fs.add_dir(root.clone(), children);

    let core = build_core(fs);
    let root_id = core.registry().register(root);

    // One shared receiver; create sessions sequentially to avoid receiver races
    let rx = core.event_receiver();

    let s1 = create_session(&core, &rx).await;
    core.send(Command::SearchNodeCompat {
        query: "stable".to_string(),
        root: root_id,
        session: s1,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    let run1: Vec<String> = drain_search(&rx, s1, LONG)
        .await
        .into_iter()
        .map(|n| n.name)
        .collect();

    let s2 = create_session(&core, &rx).await;
    core.send(Command::SearchNodeCompat {
        query: "stable".to_string(),
        root: root_id,
        session: s2,
        request: filer_core::RequestId::new(),
    })
    .unwrap();
    let run2: Vec<String> = drain_search(&rx, s2, LONG)
        .await
        .into_iter()
        .map(|n| n.name)
        .collect();

    assert_eq!(run1.len(), FILES);
    assert_eq!(
        run1, run2,
        "identical queries on the same tree must return results in the same order"
    );
}
