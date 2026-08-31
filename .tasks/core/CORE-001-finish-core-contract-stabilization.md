---
id: CORE-001
title: Finish core contract stabilization
status: Done
priority: High
type: Epic
parent: milestones:MILESTONE-003
milestone: "0.3.0"
rules: [CORE-LIBRARY, CORE-MECHANICS-BUILTIN]
risk: High
impact: "Defines the public behavior consumed by every frontend and provider."
tags: [core, contracts, stabilization]
last_updated: 2026-08-31
---

## Summary

The 0.3.0 public contracts are complete without coupling filer-core to a frontend. Wire-safe extension envelopes (MODULES-001) remain Deferred because they are not required to close this epic. Post-audit remediations that are not 0.3.0 exit gates remain under CORE-027.

## Exit Criteria

- [x] API-004 is Done (NodeId removed; LocationRef is the only addressing contract).
- [x] Canonical errors, provider timeout context, paging contracts, segmented Location routing, undo/conflict contracts, and state ownership are covered by Done child tasks.
- [x] CORE-020 is Done (shutdown, cancel, and event backpressure).
- [x] MODULES-002 is Done (in-process git decorations without blocking directory load).
- [x] REL-004 is Done (watcher burst ordering and freshness tests).
- [x] The milestone validation checklist for 0.3.0 passes.
