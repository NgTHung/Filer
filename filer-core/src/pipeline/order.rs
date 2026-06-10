//! # Pipeline Ordering
//!
//! This module defines the total row order used by incremental paging. A stable
//! path tie-breaker keeps cursor continuation deterministic when visible sort
//! fields are equal.
//!
//! ```
//! use filer_core::{FileNode, PipelineConfig};
//! use std::cmp::Ordering;
//!
//! fn is_ordered(config: &PipelineConfig, left: &FileNode, right: &FileNode) -> bool {
//!     filer_core::pipeline::compare_nodes(config, left, right) != Ordering::Greater
//! }
//! ```

use std::cmp::Ordering;
use std::time::SystemTime;

use crate::model::node::FileNode;
use crate::pipeline::config::{GroupBy, PipelineConfig};
use crate::pipeline::sort::{SortField, SortOrder};
use crate::utils;
use crate::vfs::provider::{ListingDetail, ListingOptions};

pub fn compare_nodes(config: &PipelineConfig, left: &FileNode, right: &FileNode) -> Ordering {
    let group_order = group_label(config, left).cmp(&group_label(config, right));
    if group_order != Ordering::Equal {
        return group_order;
    }

    let (field, order, directories_first) = config
        .sort
        .map(|sort| (sort.field, sort.order, sort.directories_first))
        .unwrap_or((SortField::Name, SortOrder::Ascending, true));

    if directories_first && left.is_dir() != right.is_dir() {
        return if left.is_dir() {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }

    let field_order = match field {
        SortField::Name | SortField::Type => left.name.cmp(&right.name),
        SortField::Size => left.size.cmp(&right.size),
        SortField::Modified => left.modified.cmp(&right.modified),
        SortField::Created => left.created.cmp(&right.created),
        SortField::Extension => left.extension().cmp(&right.extension()),
    };
    let field_order = match order {
        SortOrder::Ascending => field_order,
        SortOrder::Descending => field_order.reverse(),
    };

    field_order
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.path.cmp(&right.path))
}

pub fn group_label(config: &PipelineConfig, node: &FileNode) -> String {
    match config.group.map(|group| group.by).unwrap_or(GroupBy::None) {
        GroupBy::None => String::new(),
        GroupBy::Extension | GroupBy::Type => {
            node.extension().unwrap_or("No extension").to_string()
        }
        GroupBy::Date => {
            utils::time_group_name(node.modified.unwrap_or(SystemTime::UNIX_EPOCH)).to_string()
        }
        GroupBy::Size => utils::size_group_name(node.size).to_string(),
        GroupBy::FirstLetter => node
            .name
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .to_string(),
    }
}

pub fn effective_listing(config: &PipelineConfig, requested: ListingOptions) -> ListingOptions {
    let metadata_sort = config.sort.is_some_and(|sort| {
        matches!(
            sort.field,
            SortField::Size | SortField::Modified | SortField::Created
        )
    });
    let metadata_group = config
        .group
        .is_some_and(|group| matches!(group.by, GroupBy::Date | GroupBy::Size));
    let metadata_filter = config
        .filter
        .as_ref()
        .is_some_and(|filter| filter.min_size.is_some() || filter.max_size.is_some());

    if metadata_sort || metadata_group || metadata_filter {
        ListingOptions {
            detail: ListingDetail::Metadata,
        }
    } else {
        requested
    }
}
