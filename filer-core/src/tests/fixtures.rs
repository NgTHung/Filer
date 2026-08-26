//! # Test Fixtures
//!
//! Shared builders keep location registration details in one place. Use these
//! when a test needs a location-native row. Assertions should use the resulting
//! `LocationRef` or `NodeEntry` identity.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::model::location::Location;
use crate::model::node::{NodeEntry, NodeKind, NodeMeta};

pub(crate) fn local_file_node(
    path: impl Into<PathBuf>,
    name: impl Into<String>,
    kind: NodeKind,
    size: u64,
    modified: Option<SystemTime>,
    meta: NodeMeta,
) -> NodeEntry {
    let path = path.into();
    let location = Location::local(path);
    NodeEntry {
        location: crate::model::location::LocationRef::from_location(&location),
        display_path: None,
        capabilities: crate::model::node::NodeEntryCapabilities {
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

pub(crate) fn local_node_entry(node: NodeEntry) -> NodeEntry {
    node
}
