//! # Name-Keyed Row Sorting
//!
//! Sorting 10,000 rows makes about 140,000 comparisons, and deriving two name
//! keys per comparison would dominate the sort. [`KeyedSort`] derives each
//! row's key once per sort pass into one shared buffer and compares slices of
//! it, and [`KeysetBoundary`] derives the boundary row's key once for a whole
//! keyset rewalk. Both order rows exactly as
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
use crate::pipeline::name_order::{compare_keyed_names, name_key, push_name_key};
use crate::pipeline::order::compare_nodes_with;

/// Sorts rows in `compare_nodes` order. Reusing one sorter across passes also
/// reuses its buffers.
#[derive(Default)]
pub(crate) struct KeyedSort {
    keys: Vec<u8>,
    rows: Vec<KeyedRow>,
}

struct KeyedRow {
    key: Range<usize>,
    row: NodeEntry,
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
        for row in rows.drain(..) {
            let start = self.keys.len();
            push_name_key(&row.name, &mut self.keys);
            self.rows.push(KeyedRow {
                key: start..self.keys.len(),
                row,
            });
        }

        let keys = &self.keys;
        self.rows.sort_unstable_by(|left, right| {
            compare_nodes_with(config, &left.row, &right.row, || {
                compare_keyed_names(
                    &keys[left.key.clone()],
                    &left.row.name,
                    &keys[right.key.clone()],
                    &right.row.name,
                )
            })
        });
        rows.extend(self.rows.drain(..).map(|keyed| keyed.row));
    }
}

/// The last row a keyset continuation returned, with its name key derived
/// once for the whole rewalk.
pub(crate) struct KeysetBoundary {
    row: NodeEntry,
    key: Vec<u8>,
    scratch: Vec<u8>,
}

impl KeysetBoundary {
    pub(crate) fn new(row: NodeEntry) -> Self {
        let key = name_key(&row.name);
        Self {
            row,
            key,
            scratch: Vec::new(),
        }
    }

    /// Whether `entry` sorts at or before the boundary, so an earlier page
    /// already returned it.
    pub(crate) fn covers(&mut self, config: &PipelineConfig, entry: &NodeEntry) -> bool {
        let Self { row, key, scratch } = self;
        compare_nodes_with(config, entry, row, || {
            scratch.clear();
            push_name_key(&entry.name, scratch);
            compare_keyed_names(scratch, &entry.name, key, &row.name)
        })
        .is_le()
    }
}
