//! # Name-Keyed Row Sorting
//!
//! Sorting 10,000 rows makes about 140,000 comparisons, and deriving group and
//! name keys twice per comparison would dominate the sort. [`KeyedSort`]
//! derives each row's keys once per sort pass, with name keys in one shared
//! buffer, and [`KeysetBoundary`] derives the boundary row's keys once for a
//! whole keyset rewalk. Both order rows exactly as
//! [`compare_nodes`](super::compare_nodes) does.
//!
//! ```
//! use filer_core::model::node::NodeKind;
//! use filer_core::pipeline::sort::{SortBy, SortField, SortOrder};
//! use filer_core::pipeline::{PipelineData, Stage};
//! use filer_core::{Location, LocationRef, NodeEntry};
//!
//! let rows = ["file10", "file2"]
//!     .map(|name| {
//!         let location = Location::local(format!("/docs/{name}"));
//!         NodeEntry::from_location_ref(
//!             LocationRef::from_location(&location),
//!             name,
//!             NodeKind::File { extension: None },
//!         )
//!     })
//!     .to_vec();
//! let sort = SortBy::new(SortField::Name, SortOrder::Ascending, true);
//! let PipelineData::Flat(rows) = sort.process(PipelineData::Flat(rows)) else {
//!     unreachable!("sorting keeps flat data flat");
//! };
//! assert_eq!(rows[0].name, "file2");
//! ```

use std::ops::Range;

use crate::model::node::NodeEntry;
use crate::pipeline::PipelineConfig;
use crate::pipeline::name_order::{name_key, push_name_key};
use crate::pipeline::order::{GroupSortKey, RowKey, compare_keyed, group_by, group_sort_key};

/// Sorts rows in `compare_nodes` order. Reusing one sorter across passes also
/// reuses its buffers.
#[derive(Default)]
pub(crate) struct KeyedSort {
    keys: Vec<u8>,
    rows: Vec<KeyedRow>,
}

struct KeyedRow {
    name: Range<usize>,
    group: Option<GroupSortKey>,
    row: NodeEntry,
}

impl KeyedRow {
    fn key<'a>(&'a self, names: &'a [u8]) -> RowKey<'a> {
        RowKey {
            group: self.group.as_ref(),
            name: &names[self.name.clone()],
        }
    }
}

impl KeyedSort {
    pub(crate) fn sort(&mut self, config: &PipelineConfig, rows: &mut Vec<NodeEntry>) {
        self.keys.clear();
        self.rows.clear();
        // Most keys are about as long as their names, so one reservation
        // usually covers the whole pass.
        self.keys
            .reserve(rows.iter().map(|row| row.name.len()).sum::<usize>());
        self.rows.reserve(rows.len());
        let by = group_by(config);
        for row in rows.drain(..) {
            let start = self.keys.len();
            push_name_key(&row.name, &mut self.keys);
            self.rows.push(KeyedRow {
                name: start..self.keys.len(),
                group: group_sort_key(by, &row),
                row,
            });
        }

        let keys = &self.keys;
        self.rows.sort_unstable_by(|left, right| {
            compare_keyed(
                config,
                &left.row,
                left.key(keys),
                &right.row,
                right.key(keys),
            )
        });
        rows.extend(self.rows.drain(..).map(|keyed| keyed.row));
    }
}

/// The last row a keyset continuation returned, with its name key derived
/// once for the whole rewalk.
pub(crate) struct KeysetBoundary {
    row: NodeEntry,
    group: Option<GroupSortKey>,
    name: Vec<u8>,
    scratch: Vec<u8>,
}

impl KeysetBoundary {
    pub(crate) fn new(config: &PipelineConfig, row: NodeEntry) -> Self {
        Self {
            group: group_sort_key(group_by(config), &row),
            name: name_key(&row.name),
            row,
            scratch: Vec::new(),
        }
    }

    /// Whether `entry` sorts at or before the boundary, so an earlier page
    /// already returned it.
    pub(crate) fn covers(&mut self, config: &PipelineConfig, entry: &NodeEntry) -> bool {
        let group = group_sort_key(group_by(config), entry);
        self.scratch.clear();
        push_name_key(&entry.name, &mut self.scratch);
        let entry_key = RowKey {
            group: group.as_ref(),
            name: &self.scratch,
        };
        let boundary_key = RowKey {
            group: self.group.as_ref(),
            name: &self.name,
        };
        compare_keyed(config, entry, entry_key, &self.row, boundary_key).is_le()
    }
}
