//! Command-path tracing coverage (REL-002).
//!
//! Every app-facing command flows through the single dispatch choke point
//! `CommandRouter::route`, which emits one structured trace record per command.
//! This test installs a capturing subscriber and proves a record appears for
//! one command per drivable family. Coverage is structural: instrumenting the
//! one choke point covers all `Command` variants, so driving a representative
//! command per family demonstrates the guarantee end to end.
//!
//! Records fire before handler dispatch, so a bare `FilerCore::new()` (no
//! modules) is enough — the commands need only a valid session, not a working
//! downstream handler. That keeps the test deterministic with no spawned work
//! to wait on beyond the router draining its FIFO queue.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::time::{timeout, Instant};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

use filer_core::model::directory::DirectoryLoadOptions;
use filer_core::model::node::NodeId;
use filer_core::model::operation::OperationId;
use filer_core::model::request::RequestId;
use filer_core::model::session::SessionId;
use filer_core::{Command, Event, FilerCore, PipelineConfig};

const TIMEOUT: Duration = Duration::from_millis(2000);

/// One captured command record: the dispatch key and the session it carried.
#[derive(Clone)]
struct Captured {
    key: String,
    session: Option<u64>,
}

type Buffer = Arc<Mutex<Vec<Captured>>>;

struct CaptureLayer {
    buf: Buffer,
}

#[derive(Default)]
struct Grab {
    key: Option<String>,
    session: Option<u64>,
}

impl Visit for Grab {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "key" {
            self.key = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            // `key` is a &str field and normally lands in record_str; handle the
            // debug path too in case the sigil changes across tracing versions.
            "key" if self.key.is_none() => {
                self.key = Some(format!("{value:?}").trim_matches('"').to_string());
            }
            "session" => {
                self.session = parse_session_debug(&format!("{value:?}"));
            }
            _ => {}
        }
    }
}

/// Pull the inner `u64` out of a `Some(SessionId(NN))` debug string. Returns
/// `None` for the `None` session (e.g. Handshake). Parsing the debug string
/// avoids adding a test-only numeric field to the production event.
fn parse_session_debug(s: &str) -> Option<u64> {
    let start = s.find("SessionId(")? + "SessionId(".len();
    let rest = &s[start..];
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "filer_core::command" {
            return;
        }
        let mut grab = Grab::default();
        event.record(&mut grab);
        if let Some(key) = grab.key {
            self.buf.lock().unwrap().push(Captured {
                key,
                session: grab.session,
            });
        }
    }
}

static BUFFER: OnceLock<Buffer> = OnceLock::new();

/// Install the capturing subscriber once for this test binary and return the
/// shared buffer. `set_global_default` can only succeed once per process, so
/// the `OnceLock` guarantees a single install and ignores later attempts.
fn capture_buffer() -> Buffer {
    BUFFER
        .get_or_init(|| {
            let buf: Buffer = Arc::new(Mutex::new(Vec::new()));
            let subscriber =
                tracing_subscriber::registry().with(CaptureLayer { buf: buf.clone() });
            let _ = tracing::subscriber::set_global_default(subscriber);
            buf
        })
        .clone()
}

/// Send `Handshake` and return the `SessionId` from `SessionCreated`.
async fn create_session(core: &FilerCore) -> SessionId {
    let rx = core.event_receiver();
    core.send(Command::Handshake).unwrap();
    match timeout(TIMEOUT, rx.recv_async()).await {
        Ok(Ok(Event::SessionCreated(id))) => id,
        other => panic!("expected SessionCreated, got {other:?}"),
    }
}

/// True once a record with `key` (and matching `session`, when `want` is set)
/// has been captured.
fn has(records: &[Captured], key: &str, want: Option<u64>) -> bool {
    records
        .iter()
        .any(|r| r.key == key && (want.is_none() || r.session == want))
}

#[tokio::test]
async fn every_drivable_command_family_emits_a_trace_record() {
    let buf = capture_buffer();
    let core = FilerCore::new();

    // Handshake (sessionless) establishes a valid session for the rest.
    let sid = create_session(&core).await;
    let session = sid;
    let sid_num = sid.0;

    // One command per drivable family. Compat path/node variants keep
    // construction trivial; the record fires regardless of handler presence.
    let commands = vec![
        Command::NavigatePathCompat {
            path: std::path::PathBuf::from("/tmp/trace"),
            session,
            request: RequestId::new(),
        },
        Command::ScanPathCompat {
            path: std::path::PathBuf::from("/tmp/trace"),
            session,
            pipeline: PipelineConfig::default(),
            load: DirectoryLoadOptions::default(),
            request: RequestId::new(),
        },
        Command::SearchPathCompat {
            query: "needle".to_string(),
            root: std::path::PathBuf::from("/tmp/trace"),
            session,
            request: RequestId::new(),
        },
        Command::LoadPreviewNodeCompat {
            id: NodeId(0),
            options: None,
            session,
            request: RequestId::new(),
        },
        Command::CreateFolderNodeCompat {
            parent: NodeId(0),
            name: "folder".to_string(),
            session,
            request: RequestId::new(),
            operation: OperationId::new(),
        },
        Command::WatchNodeCompat {
            node: NodeId(0),
            session,
        },
    ];
    for command in commands {
        core.send(command).unwrap();
    }
    // Destroy last so the session still exists when the commands above route.
    core.send(Command::DestroySession(session)).unwrap();

    // The records fire synchronously in `route` as the router drains its FIFO
    // queue. Poll until the final command (destroy) lands, or the deadline.
    // session.destroy is the last command sent and is sessionless at the
    // command level (Command::session_id returns None for it), so wait on it
    // with no session filter — it arrives after all session-scoped records.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if has(&buf.lock().unwrap(), "session.destroy", None) {
            break;
        }
        if Instant::now() > deadline {
            let snap = buf.lock().unwrap();
            let keys: Vec<String> = snap.iter().map(|c| c.key.clone()).collect();
            panic!("timed out; captured {} records: {keys:?}", snap.len());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let records = buf.lock().unwrap().clone();
    let mine: Vec<&Captured> = records
        .iter()
        .filter(|r| r.session == Some(sid_num) || r.key == "session.handshake")
        .collect();
    assert!(!mine.is_empty(), "expected captured records for this session");

    // Handshake and DestroySession are sessionless at the command level
    // (Command::session_id returns None); the rest carry this test's session id.
    assert!(has(&records, "session.handshake", None));
    assert!(has(&records, "navigate.path.compat", Some(sid_num)));
    assert!(has(&records, "scan.path.compat", Some(sid_num)));
    assert!(has(&records, "search.path.compat", Some(sid_num)));
    assert!(has(&records, "preview.load.node.compat", Some(sid_num)));
    assert!(has(&records, "ops.create_folder.node.compat", Some(sid_num)));
    assert!(has(&records, "watch.node.compat", Some(sid_num)));
    assert!(has(&records, "session.destroy", None));

    // Not exercised here (covered structurally by the single choke point):
    // the location-native variants, cancels, metadata.*, unwatch variants,
    // and extension keys. Each still flows through `route` and is traced.
}
