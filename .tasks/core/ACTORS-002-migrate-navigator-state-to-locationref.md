---
id: ACTORS-002
title: Migrate navigator state to LocationRef
status: To Do
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
depends_on: [ACTORS-001]
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes navigation state authority and scan dispatch for session workflows."
tags: [core, actors, navigation, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Move navigator state, history, and scan dispatch toward LocationRef while keeping NodeId navigation compatibility explicit.

## Acceptance Criteria

- [ ] Navigation state and history prefer LocationRef for the current provider-aware location.
- [ ] NodeId navigation compatibility resolves once before scanner dispatch.
- [ ] Back, forward, up, and refresh preserve session boundaries.
- [ ] Navigation flow tests cover LocationRef and compatibility paths.
