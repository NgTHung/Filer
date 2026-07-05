---
id: ACTORS-003
title: Migrate preview metadata internals to LocationRef
status: Done
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes preview, metadata, and extended metadata routing before provider reads."
tags: [core, actors, preview, metadata, location, nodeid]
last_updated: 2026-07-05
---

## Summary

Move preview, metadata, and extended metadata actor internals to LocationRef while retaining NodeId compatibility at the command boundary.

## Acceptance Criteria

- [x] Preview, metadata, and extended metadata dispatch use LocationRef internally.
- [x] NodeId preview and metadata commands translate at the actor boundary.
- [x] Location-native success and failure events are authoritative.
- [x] Compatibility preview and metadata events remain where existing clients need them and are covered by tests.
