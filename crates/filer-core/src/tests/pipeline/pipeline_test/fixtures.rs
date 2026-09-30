// Tests for pipeline stages

use crate::model::node::{NodeEntry, NodeKind};
use crate::model::location::{Location, LocationRef};
use crate::pipeline::filter::{FilterByExtension, FilterHidden};
use crate::pipeline::group::{GroupBy, GroupField};
use crate::pipeline::sort::{SortBy, SortField, SortOrder};
use crate::pipeline::{
    EntryGroup, FilterConfig, GroupBy as ConfigGroupBy, GroupedEntries, Pipeline, PipelineConfig,
    PipelineData, PipelinePagingMode, SortConfig, Stage,
};
use crate::tests::fixtures::nodes;
use std::time::{Duration, SystemTime};

fn make_file(name: &str, size: u64, hidden: bool) -> NodeEntry {
    let mut entry = nodes::file(name, "/test", size);
    entry.meta.hidden = hidden;
    entry
}

fn make_file_with_ext(name: &str, ext: Option<&str>, size: u64) -> NodeEntry {
    let mut entry = nodes::file(name, "/test", size);
    entry.kind = NodeKind::File { extension: ext.map(str::to_owned) };
    entry
}

fn make_dir(name: &str, hidden: bool) -> NodeEntry {
    let mut entry = nodes::directory(name, "/test");
    entry.meta.hidden = hidden;
    entry
}

fn grouped_nodes(groups: Vec<(&str, Vec<NodeEntry>)>) -> GroupedEntries {
    let mut total_count = 0;
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(order, (label, nodes))| {
            total_count += nodes.len();
            EntryGroup {
                label: label.to_string(),
                nodes,
                order,
            }
        })
        .collect();

    GroupedEntries {
        groups,
        total_count,
    }
}
