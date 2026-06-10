---
id: MILESTONE-003
title: Core contract stabilization
status: In Progress
priority: High
type: Milestone
milestone: "0.3.0"
last_updated: 2026-06-06
---

## Summary

Stabilize the public core contracts needed by desktop, transport, provider, and extension consumers.

## Exit Criteria

- [ ] Path and NodeId command surfaces use explicit compatibility names.
- [ ] Provider calls carry timeout and capability context through app-facing errors.
- [ ] Large-directory paging works across providers with stable mutation behavior.
- [ ] Archive traversal uses segmented Location navigation.
- [ ] Undo and conflict-resolution contracts are defined.
- [ ] A trusted git decoration prototype emits semantic output without blocking directory loading.
- [ ] App-local configuration remains separate from core and ecosystem profile state.
