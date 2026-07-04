---
id: ACTORS-001
title: Migrate scanner internals to LocationRef
status: Done
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Moves scanner provider work away from transient NodeId routing."
tags: [core, actors, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Move scanner dispatch and refresh internals to LocationRef while preserving direct-local NodeId compatibility at the boundary.

## Acceptance Criteria

- [x] ScanCommand uses LocationRef for internal scan and refresh paths.
- [x] ScanNode and path compatibility inputs are translated before provider work starts.
- [x] Location-native directory events remain the authoritative scanner output.
- [x] Compatibility scan events remain only where existing clients need them and are covered by tests.
