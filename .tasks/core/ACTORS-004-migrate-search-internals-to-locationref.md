---
id: ACTORS-004
title: Migrate search internals to LocationRef
status: Done
priority: High
type: Refactor
parent: CORE-025
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Changes recursive search root authority and emitted result shape."
tags: [core, actors, search, location, nodeid]
last_updated: 2026-07-05
---

## Summary

Move recursive search internals to LocationRef while keeping NodeId and path search compatibility as explicit boundary translations.

## Acceptance Criteria

- [x] Recursive search uses LocationRef as the root authority internally.
- [x] NodeId and path search compatibility inputs translate before traversal.
- [x] Search result events prefer NodeEntry Location-native output.
- [x] Compatibility search results remain where existing clients need them and are covered by tests.
