---
id: PIPELINE-001
title: Stabilize cross-provider directory paging
status: To Do
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, PIPELINE-TRANSFORMS, ACTOR-LONG-WORK]
risk: High
impact: "Controls memory use and result consistency for large directories."
tags: [paging, pipeline, performance]
last_updated: 2026-06-06
---

## Summary

Extend paging beyond LocalFs and keep incremental views stable under mutation.

## Acceptance Criteria

- [ ] Provider types can expose native or fallback directory pages.
- [ ] Sorted and grouped pipelines support incremental loading without full snapshots.
- [ ] Page results expose virtualization hints and optional provider-native totals.
- [ ] Cursor sessions do not skip or duplicate rows when directories mutate.
- [ ] Refresh behavior has deterministic mutation and cancellation tests.
