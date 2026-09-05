//! # Integration Test Support
//!
//! These helpers keep provider-shaped fixtures separate from the
//! Location-native identities asserted by top-level integration tests.
//!
//! ```
//! use filer_core::{Location, LocationRef};
//!
//! let location = LocationRef::from_location(&Location::local("/tmp/example"));
//! assert!(location.descriptor().is_some());
//! ```

#![allow(dead_code)]

pub(crate) mod state;

use std::path::PathBuf;
use std::time::Duration;

use flume::Receiver;

use filer_core::model::node::{NodeEntry, NodeKind, NodeMeta};
use filer_core::model::session::SessionId;
use filer_core::{Event, Location, LocationRef};

pub(crate) fn local_location(path: impl Into<PathBuf>) -> LocationRef {
    LocationRef::from_location(&Location::local(path))
}

pub(crate) fn make_entry(
    path: impl Into<PathBuf>,
    name: impl Into<String>,
    kind: NodeKind,
    size: u64,
    modified: Option<std::time::SystemTime>,
    meta: NodeMeta,
) -> NodeEntry {
    let location = Location::local(path);
    NodeEntry {
        location: LocationRef::from_location(&location),
        display_path: None,
        capabilities: filer_core::NodeEntryCapabilities {
            read: true,
            navigate: matches!(kind, NodeKind::Directory { .. }),
        },
        name: name.into(),
        kind,
        size,
        modified,
        created: None,
        accessed: None,
        meta,
    }
}

pub(crate) fn provider_entry(node: NodeEntry) -> NodeEntry {
    node
}

pub(crate) async fn wait_for_directory_entries(
    events: &Receiver<Event>,
    expected_session: SessionId,
    timeout_duration: Duration,
) -> (LocationRef, usize) {
    let deadline = tokio::time::Instant::now() + timeout_duration;
    loop {
        match tokio::time::timeout_at(deadline, events.recv_async()).await {
            Ok(Ok(
                Event::DirectoryLoaded {
                    parent,
                    groups,
                    session,
                    ..
                }
                | Event::DirectoryPageLoaded {
                    parent,
                    groups,
                    session,
                    ..
                },
            )) if session == expected_session => {
                let total = groups.groups.iter().map(|group| group.nodes.len()).sum();
                return (parent, total);
            }
            Ok(Ok(Event::Error {
                message, session, ..
            })) if session == expected_session => {
                panic!("got Error instead of directory load event: {message}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed before directory load completed"),
            Err(_) => panic!("timed out waiting for native directory load event"),
        }
    }
}

pub(crate) async fn wait_for_search_entries(
    events: &Receiver<Event>,
    expected_session: SessionId,
    timeout_duration: Duration,
) -> Vec<NodeEntry> {
    let mut matches = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout_duration;
    loop {
        match tokio::time::timeout_at(deadline, events.recv_async()).await {
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
            Ok(Ok(Event::Error {
                message, session, ..
            })) if session == expected_session => {
                panic!("got Error instead of search results: {message}");
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => panic!("event channel closed before search completed"),
            Err(_) => panic!("timed out waiting for native search results"),
        }
    }
}
