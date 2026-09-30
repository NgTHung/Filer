//! # Sort Allocation Contract
//!
//! A sorted listing makes about 140,000 comparisons on 10,000 rows, so sorting
//! must derive name and group keys once per row instead of per comparison. This
//! test counts the allocations of sort passes through the public pipeline. It
//! runs in its own binary because the counting allocator is process-wide, and a
//! single test keeps other threads from adding to the count.
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
use filer_core::pipeline::{GroupBy, Pipeline, PipelineData, Stage};
use filer_core::{Location, LocationRef, NodeEntry, PipelineConfig};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const ROW_COUNT: usize = 10_000;
/// The shared key buffer and the keyed row buffer, with room for a regrowth.
const MAX_SORT_ALLOCATIONS: usize = 8;
/// Label groups allocate one label per row in the sort stage and one in the
/// grouping stage.
const MAX_GROUPED_ALLOCATIONS: usize = 2 * ROW_COUNT + 64;

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
            let name = name.as_str();
            NodeEntry::from_location_ref(
                LocationRef::from_location(&location),
                name,
                NodeKind::File {
                    extension: name.rsplit_once('.').map(|(_, ext)| ext.to_string()),
                },
            )
        })
        .collect()
}

fn allocations_of<T>(run: impl FnOnce() -> T) -> usize {
    let region = Region::new(GLOBAL);
    let output = run();
    let allocations = region.change().allocations;
    black_box(&output);
    allocations
}

#[test]
fn sorting_derives_keys_once_per_row() {
    for order in [SortOrder::Ascending, SortOrder::Descending] {
        let sort = SortBy::new(SortField::Name, order, true);
        let input = PipelineData::Flat(rows());
        let allocations = allocations_of(|| sort.process(input));
        assert!(
            allocations <= MAX_SORT_ALLOCATIONS,
            "{order:?} sort of {ROW_COUNT} rows made {allocations} allocations"
        );
    }

    let grouped = Pipeline::from_config(
        &PipelineConfig::default()
            .sort(SortField::Name, SortOrder::Ascending, true)
            .group_by(GroupBy::Extension),
    );
    let input = rows();
    let allocations = allocations_of(|| grouped.execute_grouped(input));
    assert!(
        allocations <= MAX_GROUPED_ALLOCATIONS,
        "grouped sort of {ROW_COUNT} rows made {allocations} allocations"
    );
}
