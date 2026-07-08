---
id: MILESTONE-003
title: Core contract stabilization
status: In Progress
priority: High
type: Milestone
milestone: "0.3.0"
last_updated: 2026-07-07
---

## Summary

Stabilize the public core contracts needed by desktop, transport, provider, and extension consumers.

## Exit Criteria

- [ ] Path and NodeId command surfaces use explicit compatibility names.
- [ ] Provider calls carry timeout and capability context through app-facing errors.
- [ ] Large-directory paging works across providers with stable mutation behavior.
- [ ] Archive traversal uses segmented Location navigation.
- [ ] Undo and conflict-resolution contracts are defined.
- [ ] A trusted in-process git decoration prototype (MODULES-002) emits semantic output without blocking directory loading. The wire-safe extension data plane (MODULES-001) is deferred past 0.3.0 and is not required.
- [ ] App-local configuration remains separate from core and ecosystem profile state.
- [ ] The high-severity correctness defects from the CORE-004 audit are fixed: the NodeId non-UTF-8 panic, the split listing-order authority, and the cancellation-cleanup clobber.
