---
id: CORE-006
title: Review module size and decomposition
status: Done
priority: Medium
type: Refactor
parent: CORE-004
risk: Medium
impact: "Oversized modules slow review and raise the cost of every future change in core hotspots."
tags: [core, audit, refactor]
last_updated: 2026-06-13
---

## Summary

Review files over the 700 LoC preferred ceiling and the 1000 LoC hard limit. No code change: propose split boundaries grounded in cohesion, not line count alone.

## Acceptance Criteria

- [x] Report at docs/reviews/filer-core/module-size.md inventories every file over 700 LoC.
- [x] operator.rs (1431) and mime/table.rs (1041) each have a proposed split into cohesive modules.
- [x] scanner.rs, navigator.rs, and previewer.rs each have a split recommendation or a documented reason to keep.
