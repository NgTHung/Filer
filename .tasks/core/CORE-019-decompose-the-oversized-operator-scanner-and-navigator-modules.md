---
id: CORE-019
title: Decompose the oversized operator, scanner, and navigator modules
status: To Do
priority: Medium
type: Refactor
parent: CORE-027
milestone: "0.3.1"
risk: Medium
impact: "Three core hotspots breach the size limits at real seams, raising the cost of every future change."
tags: [core, audit, remediation, refactor]
last_updated: 2026-07-09
---

## Summary

Three production modules breach the size guidance at genuine cohesion seams. Line counts refreshed 2026-07-08 after the CORE-025 LocationRef migration. operator.rs (1319, over the 1000 hard limit) splits into command vocabulary, the orchestration shell with the dispatch loop, transfer (copy/move/copy_dir_recursive), mutate (delete/rename/create), and shared plumbing; the handlers already share a skeleton and the helpers are free functions, so the move is mechanical. scanner.rs (1258, grew past the hard limit during the migration) extracts the execution core (scan_directory and its emit helpers) into scan_exec, then decomposes scan_directory into serve_from_cache, load_paged, and load_full helpers with a shared progress builder. navigator.rs (802) splits the pure NavState/NavigatorState machine from the async actor, the cleanest seam in the crate. Stage these per module; each split lands well under the limits.

Test file splits already landed in CORE-026. This task changes production modules only.

Milestone 0.3.1 (MILESTONE-004 draft). Tracked under CORE-027 post-audit remediation.

## Acceptance Criteria

- [ ] operator.rs is split into command, transfer, mutate, support, and the orchestration shell, each under 700 LoC, with no behavior change.
- [ ] scanner.rs extracts scan_exec and scan_directory is decomposed into cache, paged, and full load helpers, each under the limits.
- [ ] navigator.rs splits the NavState/NavigatorState machine into its own module from the actor.
- [ ] The existing test suites for these modules pass unchanged.
