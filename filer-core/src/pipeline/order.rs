//! # Pipeline Ordering
//!
//! This module defines the total row order used by incremental paging. A stable
//! location tie-breaker keeps cursor continuation deterministic when visible
//! sort fields are equal.
//!
//! ```
//! use filer_core::{NodeEntry, PipelineConfig};
//! use std::cmp::Ordering;
//!
//! fn is_ordered(config: &PipelineConfig, left: &NodeEntry, right: &NodeEntry) -> bool {
//!     filer_core::pipeline::compare_nodes(config, left, right) != Ordering::Greater
//! }
//! ```

use std::cmp::Ordering;

use crate::model::node::NodeEntry;
use crate::pipeline::config::{GroupBy, PipelineConfig};
use crate::pipeline::sort::{SortField, SortOrder};
use crate::utils;
use crate::vfs::provider::{ListingDetail, ListingOptions};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GroupSortKey {
    Order(u8),
    Label(String),
}

pub fn compare_nodes(config: &PipelineConfig, left: &NodeEntry, right: &NodeEntry) -> Ordering {
    let group_order = group_sort_key(config, left).cmp(&group_sort_key(config, right));
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
        .then_with(|| left.location.descriptor().cmp(&right.location.descriptor()))
        .then_with(|| left.location.identity().0.cmp(&right.location.identity().0))
}

pub fn group_label(config: &PipelineConfig, node: &NodeEntry) -> String {
    match config.group.map(|group| group.by).unwrap_or(GroupBy::None) {
        GroupBy::None => String::new(),
        GroupBy::Extension | GroupBy::Type => {
            node.extension().unwrap_or("No extension").to_string()
        }
        GroupBy::Date => utils::time_group_opt(node.modified)
            .display_name()
            .to_string(),
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

pub(crate) fn group_sort_key(config: &PipelineConfig, node: &NodeEntry) -> GroupSortKey {
    match config.group.map(|group| group.by).unwrap_or(GroupBy::None) {
        GroupBy::None => GroupSortKey::Label(String::new()),
        GroupBy::Extension | GroupBy::Type | GroupBy::FirstLetter => {
            GroupSortKey::Label(group_label(config, node))
        }
        GroupBy::Date => GroupSortKey::Order(utils::time_group_opt(node.modified).sort_order()),
        GroupBy::Size => GroupSortKey::Order(utils::size_group(node.size).sort_order()),
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
