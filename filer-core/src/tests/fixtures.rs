//! # Test Fixtures
//!
//! Shared builders keep location registration details in one place. Use these
//! when a test needs a location-native row. Assertions should use the resulting
//! `LocationRef` or `NodeEntry` identity.

#[path = "../../tests/support/state.rs"]
pub(crate) mod state;

use crate as core;
use crate::model::node::NodeEntry;

#[path = "../../tests/support/nodes.rs"]
pub(crate) mod nodes;

pub(crate) use nodes::make_entry as local_file_node;

pub(crate) fn local_node_entry(node: NodeEntry) -> NodeEntry {
    node
}
