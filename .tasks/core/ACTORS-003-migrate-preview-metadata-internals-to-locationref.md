---
id: ACTORS-003
title: Migrate preview metadata internals to LocationRef
status: To Do
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes preview, metadata, and extended metadata routing before provider reads."
tags: [core, actors, preview, metadata, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Move preview, metadata, and extended metadata actor internals to LocationRef while retaining NodeId compatibility at the command boundary.

## Acceptance Criteria

- [ ] Preview, metadata, and extended metadata dispatch use LocationRef internally.
- [ ] NodeId preview and metadata commands translate at the actor boundary.
- [ ] Location-native success and failure events are authoritative.
- [ ] Compatibility preview and metadata events remain where existing clients need them and are covered by tests.
