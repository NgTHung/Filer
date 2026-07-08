---
id: CORE-025
title: Move actor internals to LocationRef
status: To Do
priority: High
type: Epic
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Moves actor routing authority from transient node identity to canonical location identity."
tags: [core, actors, location, nodeid]
last_updated: 2026-07-08
---

## Summary

Update built-in actors and command handlers so LocationRef is the internal addressing authority. NodeId remains only at explicit compatibility boundaries while the migration is underway.

## Exit Criteria

- [x] ACTORS-001, ACTORS-002, ACTORS-003, ACTORS-004, ACTORS-005, ACTORS-006, and ACTORS-007 are Done.
- [x] Actor internals use LocationRef as the routing authority before provider work starts.
- [x] NodeId survives only at explicit compatibility command and event boundaries.
- [x] Compatibility NodeId behavior and canonical LocationRef behavior are covered by tests.
