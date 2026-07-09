---
id: OPS-002
title: Expand professional file operations
status: To Do
priority: Medium
type: Epic
milestone: "0.5.0"
depends_on: [OPS-001]
rules: [PROVIDER-ACCESS, ACTOR-LONG-WORK, CORE-MECHANICS-BUILTIN]
risk: High
impact: "Adds queued, reversible, bulk, and archive operations."
tags: [operations, queue, archive]
last_updated: 2026-07-09
---

## Summary

Build advanced operation workflows on the conflict and undo contracts.

## Exit Criteria

- [ ] Operations support queueing, history, and pause or resume where providers allow it.
- [ ] Reversible operations emit usable undo metadata.
- [ ] Bulk rename produces a validated plan before mutation.
- [ ] Archive create, extract, compress, and decompress use provider-aware operation events.
