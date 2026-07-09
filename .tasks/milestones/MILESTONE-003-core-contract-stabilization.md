---
id: MILESTONE-003
title: Core contract stabilization
status: In Progress
priority: High
type: Milestone
milestone: "0.3.0"
last_updated: 2026-07-09
---

## Summary

Stabilize the public core contracts needed by desktop, transport, provider, and extension consumers.

Open 0.3.0 work is API-004 (NodeId removal), CORE-020 (cancel/shutdown/backpressure), MODULES-002 (in-process git decorations), and REL-004 (watcher burst tests). Post-audit remediations outside this milestone live under CORE-027. The wire-safe extension data plane (MODULES-001) is deferred and is not an exit gate.

## Exit Criteria

- [ ] NodeId is removed from public and internal core contracts; LocationRef is the only addressing contract (API-004).
- [x] Provider calls carry timeout and capability context through app-facing errors (PROVIDER-001).
- [x] Cross-provider directory paging contracts landed (PIPELINE-001). Mutation-stable cursor lifetime and O(directory) next-page cost are tracked outside 0.3.0 (CORE-018 / PIPELINE-002).
- [x] Archive traversal uses segmented Location navigation (VFS-001).
- [x] Undo and conflict-resolution contracts are defined (OPS-001).
- [ ] Actor cancellation, shutdown, and high-volume event backpressure are hardened (CORE-020) so decoration and page streams do not rely on unbounded channels.
- [ ] A trusted in-process git decoration prototype (MODULES-002) emits semantic output without blocking directory loading. The wire-safe extension data plane (MODULES-001) is deferred past 0.3.0 and is not required.
- [x] App-local configuration remains separate from core and ecosystem profile state (CORE-002).
- [x] The high-severity correctness defects from the CORE-004 audit are fixed: the NodeId non-UTF-8 panic, the split listing-order authority, and the cancellation-cleanup clobber (CORE-014, CORE-015, CORE-016).
