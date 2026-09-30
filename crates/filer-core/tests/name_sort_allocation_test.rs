//! # Name Sort Allocation Contract
//!
//! A sorted listing makes about 140,000 name comparisons on 10,000 rows, so the
//! name order must not allocate per comparison or per row. This test counts the
//! allocations of one sort pass through the public pipeline stage. It runs in
//! its own binary because the counting allocator is process-wide, and a single
//! test keeps other threads from adding to the count.
//!
//! ```
//! use filer_core::pipeline::sort::{SortBy, SortField, SortOrder};
//!
//! let sort = SortBy::new(SortField::Name, SortOrder::Ascending, true);
//! # let _ = sort;
//! ```

use std::alloc::System;
use std::hint::black_box;

use filer_core::model::node::NodeKind;
use filer_core::pipeline::sort::{SortBy, SortField, SortOrder};
use filer_core::pipeline::{PipelineData, Stage};
use filer_core::{Location, LocationRef, NodeEntry};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const ROW_COUNT: usize = 10_000;
/// The shared key buffer and the keyed row buffer, with room for a regrowth.
const MAX_SORT_ALLOCATIONS: usize = 8;

fn rows() -> Vec<NodeEntry> {
    (0..ROW_COUNT)
        .rev()
        .map(|index| {
            let name = match index % 3 {
                0 => format!("IMG_{index:05}.png"),
                1 => format!("Tài liệu {index}.md"),
                _ => format!("file{index}.txt"),
            };
            let location = Location::local(format!("/name-sort/{name}"));
            NodeEntry::from_location_ref(
                LocationRef::from_location(&location),
                name,
                NodeKind::File { extension: None },
            )
        })
        .collect()
}

#[test]
fn name_sort_derives_keys_without_per_row_allocations() {
    for order in [SortOrder::Ascending, SortOrder::Descending] {
        let sort = SortBy::new(SortField::Name, order, true);
        let input = PipelineData::Flat(rows());

        let region = Region::new(GLOBAL);
        let output = sort.process(input);
        let allocations = region.change().allocations;
        black_box(&output);

        assert!(
            allocations <= MAX_SORT_ALLOCATIONS,
            "{order:?} sort of {ROW_COUNT} rows made {allocations} allocations"
        );
    }
}
