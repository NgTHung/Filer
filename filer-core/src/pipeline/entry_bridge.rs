//! # NodeEntry Pipeline Bridge
//!
//! This private module keeps the old `FileNode` pipeline usable while provider
//! and paging APIs carry `NodeEntry`. It restores the original location-native
//! row after each pipeline operation so API-016 can remove this boundary.
//!
//! ```ignore
//! let groups = entry_bridge::execute_grouped_entries(entries, &config);
//! ```

use std::collections::{HashMap, VecDeque};

use crate::model::node::{FileNode, NodeEntry, NodeId};

use super::{EntryGroup, GroupedEntries, GroupedNodes, Pipeline, PipelineConfig};

/// Convert a provider row at the private compatibility boundary.
pub(crate) fn to_file_node(entry: &NodeEntry) -> FileNode {
    entry.to_file_node()
}

pub(crate) fn to_file_nodes(entries: &[NodeEntry]) -> Vec<FileNode> {
    entries.iter().map(to_file_node).collect()
}

/// Return cache rows only when the temporary row bridge cannot change identity.
pub(crate) fn cacheable_file_nodes(entries: &[NodeEntry]) -> Option<Vec<FileNode>> {
    let nodes = to_file_nodes(entries);
    let preserves_identity = entries.iter().zip(&nodes).all(|(entry, node)| {
        entry
            .location
            .descriptor()
            .and_then(|descriptor| descriptor.as_local_path())
            .is_some_and(|path| path == node.path.as_path())
    });
    preserves_identity.then_some(nodes)
}

/// Run the legacy FileNode pipeline while retaining the provider-owned row.
pub(crate) fn execute_grouped_entries(
    entries: Vec<NodeEntry>,
    pipeline_config: &PipelineConfig,
) -> GroupedEntries {
    let mut by_identity = HashMap::<(NodeId, std::path::PathBuf), VecDeque<NodeEntry>>::new();
    let nodes = entries
        .into_iter()
        .map(|entry| {
            let node = to_file_node(&entry);
            by_identity
                .entry((entry.id, node.path.clone()))
                .or_default()
                .push_back(entry);
            node
        })
        .collect();
    let grouped = Pipeline::from_config(pipeline_config).execute_grouped(nodes);
    grouped_entries(grouped, by_identity)
}

fn grouped_entries(
    grouped: GroupedNodes,
    mut by_identity: HashMap<(NodeId, std::path::PathBuf), VecDeque<NodeEntry>>,
) -> GroupedEntries {
    let total_count = grouped.total_count;
    GroupedEntries {
        groups: grouped
            .groups
            .into_iter()
            .map(|group| EntryGroup {
                label: group.label,
                nodes: group
                    .nodes
                    .into_iter()
                    .filter_map(|node| {
                        by_identity
                            .get_mut(&(node.id, node.path))
                            .and_then(VecDeque::pop_front)
                    })
                    .collect(),
                order: group.order,
            })
            .collect(),
        total_count,
    }
}
