---
id: CORE-015
title: Unify directory listing and group ordering on one comparator
status: Done
priority: High
type: Bug
parent: CORE-001
milestone: "0.3.0"
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "Two disagreeing order authorities break cursor stability and show the same directory in different orders."
tags: [core, audit, remediation, pipeline]
last_updated: 2026-07-04
---

## Summary

Listing order is defined twice. The snapshot path orders through SortBy::sort_nodes; the paged path orders through compare_nodes. They disagree on tie-breaking (compare_nodes has a total name-then-path tie-break, SortBy leans on input order) and on extension direction (compare_nodes sorts extensionless first, SortBy last). Cursor stability rests entirely on compare_nodes, which the snapshot path does not use. Separately, group display order keys on the label string and ignores TimeGroup/SizeGroup::sort_order, so date and size groups render alphabetically rather than logically. Make compare_nodes the single comparator both paths call, and order groups by sort_order.

## Acceptance Criteria

- [x] The SortBy stage and the paged path both order through compare_nodes; SortBy no longer carries independent ordering logic.
- [x] Extension and tie-break ordering is identical between a snapshot load and a paged load of the same directory and config, pinned by a test.
- [x] Date and size groups order by TimeGroup::sort_order and SizeGroup::sort_order in both group.rs and the compare_nodes group key, pinned by a test.
