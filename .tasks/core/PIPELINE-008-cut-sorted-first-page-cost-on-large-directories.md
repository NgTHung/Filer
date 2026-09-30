---
id: "PIPELINE-008"
title: "Cut sorted first-page cost on large directories"
status: Done
priority: "High"
type: "Refactor"
milestone: "0.3.1"
depends_on: ["PIPELINE-009", "PIPELINE-010"]
rules: ["PIPELINE-TRANSFORMS"]
risk: "Low"
impact: "Sorted listing is the default browse path and currently costs about ten times the unsorted full walk."
tags: ["core", "performance", "pipeline", "enhancement", "ready-for-agent"]
last_updated: 2026-09-30
---

## Summary

The 2026-09-05 CORE-021 baseline records a sorted first page at 101 ms median with 509,092 allocations on 10,000 entries. The full unsorted snapshot of the same directory takes 9.4 ms with 100,380 allocations. The default name sort is a byte comparison, so comparator work alone does not explain the gap. Profile the sorted PageSelection path, attribute the cost to call sites, and remove it. Where a comparison needs derived data, such as group labels in pipeline/order.rs, derive it once per row instead of once per comparison. The PIPELINE-009 name_order benchmark found that compare_nodes derives group sort keys twice per comparison even for ungrouped listings: skipping them cut a 10,000-row sort from 12.4 ms to 1.8 ms (docs/adr/0002-default-name-order.md). PIPELINE-010 ships the default name order that PIPELINE-009 chose before this task starts, so the before and after runs compare the order that ships. User-selectable and locale-aware modes stay with PIPELINE-002. Preserve PIPELINE-003 lookahead, ordered continuation, and cancellation contracts.

## Acceptance Criteria

- [x] An allocation and time profile of the sorted first page attributes the gap to named call sites, and the findings are recorded with the benchmark baseline.
- [x] Grouped sorting derives each row's group key once per sort pass instead of allocating a label per comparison.
- [x] Regression tests prove sorted and grouped output, flat and paged parity, lookahead, continuation, and cancellation are unchanged.
- [x] A same-machine before/after run on the 10,000-entry fixture records sorted first-page time and allocations; sorted first-page p95 is within 2x of full-snapshot p95, or the report names the remaining cost and why it stays.

## Rationale

Added on 2026-09-23 after comparing Filer with Filesmash, a native Win32 file manager. Explorer-style clients open folders sorted by default, so this path decides perceived folder-open speed.
