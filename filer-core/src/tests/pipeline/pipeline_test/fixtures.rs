// Tests for pipeline stages

use crate::model::node::{NodeEntry, NodeKind, NodeMeta};
use crate::model::location::{Location, LocationRef};
use crate::pipeline::filter::{FilterByExtension, FilterHidden};
use crate::pipeline::group::{GroupBy, GroupField};
use crate::pipeline::sort::{SortBy, SortField, SortOrder};
use crate::pipeline::{
    EntryGroup, FilterConfig, GroupBy as ConfigGroupBy, GroupedEntries, Pipeline, PipelineConfig,
    PipelineData, PipelinePagingMode, SortConfig, Stage,
};
use crate::tests::fixtures::local_file_node;
use crate::utils;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn make_file(name: &str, size: u64, hidden: bool) -> NodeEntry {
    let path = PathBuf::from(format!("/test/{name}"));
    let extension = utils::get_extension(PathBuf::from(name).as_path()).map(str::to_string);
    local_file_node(
        path,
        name,
        NodeKind::File { extension },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    )
}

fn make_file_with_ext(name: &str, ext: Option<&str>, size: u64) -> NodeEntry {
    let path = PathBuf::from(format!("/test/{name}"));
    local_file_node(
        path,
        name,
        NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        NodeMeta {
            hidden: false,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    )
}

fn make_dir(name: &str, hidden: bool) -> NodeEntry {
    let path = PathBuf::from(format!("/test/{name}"));
    local_file_node(
        path,
        name,
        NodeKind::Directory {
            children_count: None,
        },
        0,
        Some(SystemTime::UNIX_EPOCH),
        NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
    )
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
