---
id: CORE-019
title: Decompose the oversized operator, scanner, and navigator modules
status: To Do
priority: Medium
type: Refactor
parent: CORE-001
risk: Medium
impact: "Three core hotspots breach the size limits at real seams, raising the cost of every future change."
tags: [core, audit, remediation, refactor]
last_updated: 2026-07-04
---

## Summary

Three production modules breach the size guidance at genuine cohesion seams. operator.rs (1431, over the 1000 hard limit) splits into command vocabulary, the orchestration shell with the dispatch loop, transfer (copy/move/copy_dir_recursive), mutate (delete/rename/create), and shared plumbing; the handlers already share a skeleton and the helpers are free functions, so the move is mechanical. scanner.rs (994) extracts the execution core (scan_directory and its emit helpers) into scan_exec, then decomposes the 530-line scan_directory into serve_from_cache, load_paged, and load_full helpers with a shared progress builder. navigator.rs (707) splits the pure NavState/NavigatorState machine from the async actor, the cleanest seam in the crate. Stage these per module; each split lands well under the limits.

## Acceptance Criteria

- [ ] operator.rs is split into command, transfer, mutate, support, and the orchestration shell, each under 700 LoC, with no behavior change.
- [ ] scanner.rs extracts scan_exec and scan_directory is decomposed into cache, paged, and full load helpers, each under the limits.
- [ ] navigator.rs splits the NavState/NavigatorState machine into its own module from the actor.
- [ ] The existing test suites for these modules pass unchanged, and oversized test files are split along the same seams.
