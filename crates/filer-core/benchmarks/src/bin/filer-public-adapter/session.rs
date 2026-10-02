//! # Filer-core session
//!
//! This module talks to Filer-core only through `Command` and `Event`, the
//! contract a real client uses. Each scan carries a fresh request id and
//! accepts a result only when both its session and request match, so a stale
//! or foreign event can never complete a benchmark milestone.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use filer_core::model::node::NodeKind;
use filer_core::model::session::SessionId;
use filer_core::{
    Command, DirectoryCursor, DirectoryLoadMode, DirectoryLoadOptions, Event, FilerCore,
    ListingOptions, Location, LocationRef, NodeEntry, PipelineConfig, RequestId,
};
use filer_core_benchmarks::{CanonicalRow, Kind};

const EVENT_TIMEOUT: Duration = Duration::from_secs(60);

/// A failure that ends the sample with an `error` status.
#[derive(Debug)]
pub struct AdapterFailure {
    pub code: &'static str,
    pub message: String,
}

impl AdapterFailure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub struct Page {
    pub rows: Vec<CanonicalRow>,
    pub next_cursor: Option<DirectoryCursor>,
}

pub struct CoreSession {
    core: FilerCore,
    events: flume::Receiver<Event>,
    session: SessionId,
    location: LocationRef,
    metadata: bool,
    received_events: u64,
}

impl CoreSession {
    /// Starts the default core and opens a session; this is untimed setup.
    pub async fn open(fixture_root: &Path, metadata: bool) -> Result<Self, AdapterFailure> {
        let core = FilerCore::with_defaults();
        let events = core.event_receiver();
        core.send(Command::Handshake)
            .map_err(|error| AdapterFailure::new("core_error", error.to_string()))?;
        let session = loop {
            if let Event::SessionCreated(session) = receive(&events).await? {
                break session;
            }
        };
        Ok(Self {
            core,
            events,
            session,
            location: LocationRef::from_location(&Location::local(fixture_root)),
            metadata,
            received_events: 0,
        })
    }

    /// Public core events received since the session opened.
    pub fn received_events(&self) -> u64 {
        self.received_events
    }

    /// Requests one provider-order page and waits for its correlated result.
    pub async fn scan_page(
        &mut self,
        page_size: usize,
        cursor: Option<DirectoryCursor>,
    ) -> Result<Page, AdapterFailure> {
        let request = RequestId::new();
        let listing = if self.metadata {
            ListingOptions::metadata()
        } else {
            ListingOptions::fast()
        };
        self.core
            .send(Command::Scan {
                location: self.location.clone(),
                session: self.session,
                pipeline: PipelineConfig::default(),
                load: DirectoryLoadOptions {
                    listing,
                    mode: DirectoryLoadMode::Page {
                        limit: page_size,
                        cursor,
                    },
                },
                request,
            })
            .map_err(|error| AdapterFailure::new("core_error", error.to_string()))?;
        loop {
            let event = receive(&self.events).await?;
            self.received_events += 1;
            match event {
                Event::DirectoryPageLoaded {
                    groups,
                    page,
                    session,
                    request: event_request,
                    ..
                } if session == self.session && event_request == request => {
                    let rows = groups
                        .groups
                        .iter()
                        .flat_map(|group| &group.nodes)
                        .map(|entry| self.canonical_row(entry))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Page {
                        rows,
                        next_cursor: page.next_cursor,
                    });
                }
                Event::Error {
                    code,
                    message,
                    session,
                    request: Some(event_request),
                    ..
                } if session == self.session && event_request == request => {
                    return Err(AdapterFailure::new(
                        "core_error",
                        format!("{code:?}: {message}"),
                    ));
                }
                Event::DirectoryLoaded {
                    session,
                    request: event_request,
                    ..
                } if session == self.session && event_request == request => {
                    return Err(AdapterFailure::new(
                        "unexpected_core_event",
                        "a paged scan returned a snapshot",
                    ));
                }
                _ => {}
            }
        }
    }

    /// Cancels any scan still running for this session, then stops every actor.
    pub async fn close(self) -> Result<(), AdapterFailure> {
        let cancelled = self
            .core
            .send(Command::CancelScan {
                session: self.session,
            })
            .and_then(|()| self.core.send(Command::DestroySession(self.session)));
        let shutdown = self.core.shutdown().await;
        cancelled
            .and(shutdown)
            .map_err(|error| AdapterFailure::new("core_shutdown_failed", error.to_string()))
    }

    fn canonical_row(&self, entry: &NodeEntry) -> Result<CanonicalRow, AdapterFailure> {
        let (kind, size_bytes) = match entry.kind {
            NodeKind::File { .. } => (Kind::File, Some(entry.size)),
            NodeKind::Directory { .. } => (Kind::Directory, None),
            NodeKind::Symlink { .. } => {
                return Err(AdapterFailure::new(
                    "invalid_entry",
                    format!(
                        "{} is a symlink, which flat fixtures do not contain",
                        entry.name
                    ),
                ));
            }
        };
        // Fast listings do not collect metadata; the trace omits these fields.
        let modified_unix_ns = if self.metadata {
            let modified = entry.modified.ok_or_else(|| {
                AdapterFailure::new(
                    "invalid_entry",
                    format!("{} has no modification time", entry.name),
                )
            })?;
            unix_ns(modified).ok_or_else(|| {
                AdapterFailure::new(
                    "invalid_entry",
                    format!("{} has an unrepresentable modification time", entry.name),
                )
            })?
        } else {
            0
        };
        Ok(CanonicalRow::new(
            entry.name.clone(),
            kind,
            size_bytes,
            modified_unix_ns,
        ))
    }
}

async fn receive(events: &flume::Receiver<Event>) -> Result<Event, AdapterFailure> {
    tokio::time::timeout(EVENT_TIMEOUT, events.recv_async())
        .await
        .map_err(|_| AdapterFailure::new("core_timeout", "timed out waiting for a core event"))?
        .map_err(|_| AdapterFailure::new("core_error", "core event channel closed"))
}

fn unix_ns(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).ok(),
        Err(before) => i64::try_from(before.duration().as_nanos())
            .ok()
            .map(|nanos| -nanos),
    }
}
