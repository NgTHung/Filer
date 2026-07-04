---
id: ACTORS-005
title: Migrate operation internals to LocationRef
status: To Do
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes write-operation routing and affected-target events."
tags: [core, actors, operations, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Move copy, move, delete, rename, create-file, and create-folder operation internals to LocationRef while retaining explicit NodeId compatibility.

## Acceptance Criteria

- [ ] Copy, move, delete, rename, create-file, and create-folder use LocationRef internally.
- [ ] NodeId operation commands translate at the actor boundary.
- [ ] Operation completion emits LocationRef-native affected targets as authoritative output.
- [ ] Compatibility operation completion remains where existing clients need it and is covered by tests.
