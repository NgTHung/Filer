---
id: ACTORS-007
title: Audit LocationRef actor compatibility boundaries
status: Done
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
depends_on: [ACTORS-001, ACTORS-002, ACTORS-003, ACTORS-004, ACTORS-005, ACTORS-006]
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Confirms NodeId compatibility remains explicit before public API deprecation starts."
tags: [core, actors, compatibility, location, nodeid]
last_updated: 2026-07-08
---

## Summary

Audit migrated actors so NodeId survives only as explicit compatibility input or output and LocationRef-native events are authoritative.

## Acceptance Criteria

- [x] No actor performs provider work from unresolved NodeId input.
- [x] NodeId variants remain only as public compatibility inputs or compatibility outputs.
- [x] Tests prove canonical LocationRef flows and retained compatibility flows.
- [x] CORE-025 exit criteria can be checked after this task is complete.
