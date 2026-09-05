---
id: "OPS-003"
title: "Specify validated bulk rename as the first advanced operation"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:OPS-002"
milestone: "0.5.0"
depends_on: ["milestones:MILESTONE-005", "core:OPS-001"]
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["core"]
last_updated: "2026-09-05"
---

## Summary

Choose bulk rename planning as the first OPS-002 slice. Build on OperationConflictPolicy, OperationId, and OperationUndoRecord. Specify a deterministic plan and validation before mutation, then execution through existing operation events. Queue/pause/history and archive workflows remain later stages.

## Acceptance Criteria

- [ ] Plan validation covers duplicate targets, collisions, rename cycles, unsupported capabilities, and changes between plan and execution.
- [ ] Execution semantics cover cancellation, partial failure, provider guarantees, and honest undo metadata without promising unsupported atomicity.
- [ ] Separate test-first tickets cover planning and execution; remaining OPS-002 outcomes retain a stage map and concrete dependencies when refined.
