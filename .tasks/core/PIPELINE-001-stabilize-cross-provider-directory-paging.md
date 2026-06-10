---
id: PIPELINE-001
title: Stabilize cross-provider directory paging
status: Done
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, PIPELINE-TRANSFORMS, ACTOR-LONG-WORK]
risk: High
impact: "Controls memory use and result consistency for large directories."
tags: [paging, pipeline, performance]
last_updated: 2026-06-10
---

## Summary

Extend paging beyond LocalFs and keep incremental views stable between refreshes.
An active cursor chain must not duplicate or skip unchanged rows. Refresh starts
a new authoritative view that incorporates concurrent inserts, deletes, and
renames.

## Acceptance Criteria

- [x] Provider types can expose native or fallback directory pages.
- [x] Sorted and grouped pipelines support incremental loading without full snapshots.
- [x] Page results expose virtualization hints and optional provider-native totals.
- [x] Cursor sessions do not skip or duplicate unchanged rows before refresh.
- [x] Refresh behavior has deterministic mutation and cancellation tests.
