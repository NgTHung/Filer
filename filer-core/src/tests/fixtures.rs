//! # Test Fixtures
//!
//! Shared builders keep location registration details in one place. Use these
//! when a test needs a `FileNode` row but the assertion is about location or
//! query behavior. The `NodeId` field is provider-boundary plumbing only; tests
//! must assert on the resulting `LocationRef` or `NodeEntry`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::model::location::Location;
use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::model::registry::NodeRegistry;

pub(crate) fn registered_local_node_id(path: impl AsRef<Path>) -> NodeId {
    let registry = NodeRegistry::new();
    registry
        .register_location_node(Location::local(path.as_ref().to_path_buf()))
        .unwrap()
}

pub(crate) fn local_file_node(
    path: impl Into<PathBuf>,
    name: impl Into<String>,
    kind: NodeKind,
    size: u64,
    modified: Option<SystemTime>,
    meta: NodeMeta,
) -> FileNode {
    let path = path.into();
    FileNode {
        id: registered_local_node_id(&path),
        name: name.into(),
        path,
        kind,
        size,
        modified,
        created: None,
        accessed: None,
        meta,
    }
}
