---
id: CORE-021
title: Cut pipeline hot-path allocation on the large-directory scan
status: To Do
priority: Medium
type: Refactor
parent: CORE-001
rules: [PIPELINE-TRANSFORMS]
risk: Low
impact: "Per-row allocation and dead stage work on the scan hot path tax the performance-first priority."
tags: [core, audit, remediation, performance]
last_updated: 2026-07-04
---

## Summary

The grouped pipeline branches clone every node they touch: FilterHidden, FilterByExtension, and SortBy all do group.nodes = self.xxx(group.nodes.clone()), a deep per-row Vec<FileNode> allocation that std::mem::take(&mut group.nodes) removes. Separately, PageSelection::extend calls execute_flat(vec![entry]) once per directory row, allocating a one-element Vec and walking every stage for a single node, while the real order comes from the compare_nodes binary insert that follows, so the SortBy and GroupBy stages do nothing useful per row. On the large-directory target this is per-row allocation plus dead stage work on the hot path. Remove the grouped-branch clones via mem::take and avoid the per-row full-pipeline execution.

## Acceptance Criteria

- [ ] Grouped filter and sort stages move their input with mem::take instead of cloning, with no behavior change.
- [ ] The paging hot path no longer runs the full pipeline per row; per-row filtering keeps only the work that affects selection.
- [ ] A benchmark or test demonstrates reduced per-row allocation on a large-directory scan.
