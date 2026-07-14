---
id: CORE-001
title: Finish core contract stabilization
status: In Progress
priority: High
type: Epic
parent: milestones:MILESTONE-003
milestone: "0.3.0"
rules: [CORE-LIBRARY, CORE-MECHANICS-BUILTIN]
risk: High
impact: "Defines the public behavior consumed by every frontend and provider."
tags: [core, contracts, stabilization]
last_updated: 2026-07-09
---

## Summary

Complete the remaining 0.3.0 public contracts without coupling filer-core to a frontend.

Remaining open children for this epic: API-004 (and its staged children), CORE-020, MODULES-002, and REL-004. Wire-safe extension envelopes (MODULES-001) are Deferred and are not required to close this epic. Post-audit remediations that are not 0.3.0 exit gates live under CORE-027.

## Exit Criteria

- [ ] API-004 is Done (NodeId removed; LocationRef is the only addressing contract).
- [x] Canonical errors, provider timeout context, paging contracts, segmented Location routing, undo/conflict contracts, and state ownership are covered by Done child tasks.
- [ ] CORE-020 is Done (shutdown, cancel, and event backpressure).
- [ ] MODULES-002 is Done (in-process git decorations without blocking directory load).
- [ ] REL-004 is Done (watcher burst ordering and freshness tests).
- [ ] The milestone validation checklist for 0.3.0 passes.
