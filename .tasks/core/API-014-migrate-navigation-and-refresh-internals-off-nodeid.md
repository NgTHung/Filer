---
id: "API-014"
title: "Migrate navigation and refresh internals off NodeId"
status: "To Do"
priority: "High"
type: "Refactor"
parent: "API-007"
milestone: "0.3.0"
depends_on: ["API-006"]
rules: ["CORE-LIBRARY", "PROVIDER-ACCESS"]
risk: "High"
impact: "Moves navigation, selection, refresh invalidation, and progress targets to Location identity before row and registry removal."
tags: ["api", "nodeid", "location"]
last_updated: "2026-08-14"
---

## Summary

Replace NodeId navigation state and refresh plumbing with normalized LocationRef values and LocationId keys, while preserving navigation and session behavior.

## Acceptance Criteria

- [ ] Navigation state, history, selection, watcher invalidation, and progress targets contain no NodeId.
- [ ] ScanCompat, RefreshCompat, and dead internal NodeId and path navigation variants are removed.
- [ ] Focused navigation, watcher, operation, serialization, and full filer-core tests pass.
