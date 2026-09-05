---
id: "OPS-002"
title: "Expand professional file operations"
status: "To Do"
priority: "Medium"
type: "Epic"
milestone: "0.5.0"
depends_on: ["OPS-001"]
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK", "CORE-MECHANICS-BUILTIN"]
risk: "High"
impact: "Adds queued, reversible, bulk, and archive operations."
tags: ["operations", "queue", "archive", "enhancement", "needs-triage"]
last_updated: "2026-09-05"
---

## Summary

Build advanced operation workflows on the conflict and undo contracts.

OPS-003 specifies validated bulk rename first and creates separate planning and execution tickets. Queue/history/pause and archive operations remain later stages, refined against actual provider guarantees. Create their implementation tickets before starting those stages; this epic is an outcome map rather than a single implementation assignment.

## Exit Criteria

- [ ] Operations support queueing, history, and pause or resume where providers allow it.
- [ ] Reversible operations emit usable undo metadata.
- [ ] Bulk rename produces a validated plan before mutation.
- [ ] Archive create, extract, compress, and decompress use provider-aware operation events.
