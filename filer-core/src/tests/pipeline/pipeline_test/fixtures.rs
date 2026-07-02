// Tests for pipeline stages

use crate::model::node::{FileNode, NodeId, NodeKind, NodeMeta};
use crate::pipeline::filter::{FilterByExtension, FilterHidden};
use crate::pipeline::group::{GroupBy, GroupField};
use crate::pipeline::sort::{SortBy, SortField, SortOrder};
use crate::pipeline::{
    FileGroup, FilterConfig, GroupBy as ConfigGroupBy, GroupedNodes, Pipeline, PipelineConfig,
    PipelineData, PipelinePagingMode, SortConfig, Stage,
};
use crate::utils;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

// Helper to create test FileNodes
fn make_file(name: &str, size: u64, hidden: bool) -> FileNode {
    let extension = utils::get_extension(PathBuf::from(name).as_path()).map(str::to_string);
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::File { extension },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
        created: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
        accessed: None,
    }
}

fn make_file_with_ext(name: &str, ext: Option<&str>, size: u64) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::File {
            extension: ext.map(|s| s.to_string()),
        },
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
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

fn make_dir(name: &str, hidden: bool) -> FileNode {
    FileNode {
        id: NodeId(name.len() as u64 + 1000),
        name: name.to_string(),
        path: PathBuf::from(format!("/test/{}", name)),
        kind: NodeKind::Directory {
            children_count: None,
        },
        size: 0,
        modified: Some(SystemTime::UNIX_EPOCH),
        created: None,
        meta: NodeMeta {
            hidden,
            readonly: false,
            permissions: None,
            ..Default::default()
        },
        accessed: None,
    }
}

fn grouped_nodes(groups: Vec<(&str, Vec<FileNode>)>) -> GroupedNodes {
    let mut total_count = 0;
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(order, (label, nodes))| {
            total_count += nodes.len();
            FileGroup {
                label: label.to_string(),
                nodes,
                order,
            }
        })
        .collect();

    GroupedNodes {
        groups,
        total_count,
    }
}

