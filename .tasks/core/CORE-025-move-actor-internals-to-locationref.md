---
id: CORE-025
title: Move actor internals to LocationRef
status: To Do
priority: High
type: Refactor
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: High
impact: "Moves actor routing authority from transient node identity to canonical location identity."
tags: [core, actors, location, nodeid]
last_updated: 2026-07-04
---

## Summary

Update built-in actors and command handlers so LocationRef is the internal addressing authority. NodeId remains only at explicit compatibility boundaries while the migration is underway.

## Acceptance Criteria

- [ ] Scanner, navigator, preview, metadata, search, watcher, and operation actors accept or resolve LocationRef at the dispatch boundary before doing provider work.
- [ ] NodeId command variants are translated into LocationRef once, then internal actor paths operate on Location or LocationRef.
- [ ] Events emitted from migrated flows include LocationRef-native variants as the authoritative output.
- [ ] Compatibility NodeId events remain only where needed for existing clients and are covered by tests.
