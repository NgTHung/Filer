---
id: CORE-010
title: Review data model and pipeline contracts
status: Done
priority: Medium
type: Design
parent: CORE-004
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "The model and pipeline contracts are consumed by every frontend; weak contracts propagate widely."
tags: [core, audit, model, pipeline]
last_updated: 2026-06-13
---

## Summary

Review the Location/Node/Query types, Pipeline transforms, the GroupedNodes contract, and cursor stability under mutation.

## Acceptance Criteria

- [x] Report at docs/reviews/filer-core/model-pipeline.md evaluates the core data types and pipeline transform contract.
- [x] Cursor stability under directory mutation is assessed with the failure modes documented.
- [x] Follow-up task candidates are listed for contract weaknesses.
