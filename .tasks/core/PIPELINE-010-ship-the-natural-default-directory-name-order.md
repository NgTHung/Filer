---
id: "PIPELINE-010"
title: "Ship the natural default directory name order"
status: "To Do"
priority: "High"
type: "Bug"
milestone: "0.3.1"
depends_on: ["PIPELINE-009"]
rules: ["PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Changes the order of every sorted listing and ordered continuation."
tags: ["pipeline", "sorting", "paging", "bug", "ready-for-agent"]
last_updated: "2026-09-30"
---

## Summary

Name sort compares raw bytes in crates/filer-core/src/pipeline/order.rs. ADR 0002 (docs/adr/0002-default-name-order.md) chooses a case-insensitive, number-aware order with a leading-zero step and a raw-byte tie-break, and defines it with one key function. Implement that key in a new pipeline module. Bulk sorts in SortBy and PageSelection derive each row's key once into a shared buffer, and keyset continuations compare walked rows against the boundary row's key. Descending Name reverses the whole name order, and the name order breaks ties for every other sort field. Extension values, group labels, and user-selectable or locale-aware modes stay with PIPELINE-002. The group-key cost inside compare_nodes belongs to PIPELINE-008.

## Acceptance Criteria

- [ ] A new module under crates/filer-core/src/pipeline/ defines the name key, and tests cover every example pair in ADR 0002, including mixed case, digit runs, leading zeros, digit runs longer than 20 digits, and non-ASCII names.
- [ ] A test proves the name order is a strict total order over every triple of an edge-case set that includes U+0130, the Kelvin sign, ß, and the empty name.
- [ ] SortBy and PageSelection derive each row's name key once per sort pass into one shared buffer, and keyset continuations derive the boundary row's key once.
- [ ] Tests prove descending Name is the exact reverse of ascending Name, and equal Size, Modified, Created, and Extension values fall back to ascending name order.
- [ ] Regression tests prove flat and paged parity, grouped output, lookahead, continuation, and cancellation hold under the new order.
- [ ] The name_order benchmark measures the shipped key path instead of its prototype candidates and reports no per-row key allocations, and a same-machine large_directory run records sorted first-page time and allocations before and after the change.
