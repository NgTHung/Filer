---
id: ACTORS-006
title: Migrate watcher internals to LocationRef
status: To Do
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes watch identity, refresh routing, and filesystem change events."
tags: [core, actors, watcher, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Move watcher keys, watch entries, and change events to LocationRef while keeping NodeId watch compatibility explicit.

## Acceptance Criteria

- [ ] Watch keys and watch entries use LocationRef as the authoritative identity.
- [ ] NodeId watch and unwatch compatibility inputs translate before provider watch setup.
- [ ] Filesystem change events emit LocationRef-native variants.
- [ ] Compatibility filesystem change events remain only where required and are covered by tests.
