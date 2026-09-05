---
id: CORE-019
title: Decompose the oversized operator, scanner, and navigator modules
status: To Do
priority: Medium
type: Refactor
parent: CORE-027
milestone: "0.3.1"
risk: Medium
impact: "Separates the remaining oversized operation and scan modules at existing behavior boundaries."
tags: [core, audit, remediation, refactor]
last_updated: 2026-09-05
---

## Summary

The 2026-09-05 inventory finds operator.rs at 1,310 lines and scanner.rs at 1,067 lines. Navigation already separates navigator.rs (411 lines) from state.rs (201 lines), which exports NavState and NavigatorState. Remaining work is staged as CORE-035 for operations and CORE-036 for scanning. Each stage preserves public contracts and lands independently.

Test file splits already landed in CORE-026. This task changes production modules only.

Keep mechanical moves separate from control-flow changes. Inspect the actual diff before each commit; complex changes stay below 700 changed lines and larger moves need a documented mechanical-only rationale.

## Acceptance Criteria

- [ ] CORE-035 is Done: operation command, transfer, mutation, and shared support responsibilities are separated, with modules under 700 lines and no behavior change.
- [ ] CORE-036 is Done: scanner orchestration and cache, paged, and full execution responsibilities are separated into modules under 700 lines.
- [x] navigator.rs splits the NavState/NavigatorState machine into its own module from the actor; verified against navigation/state.rs on 2026-09-05.
- [ ] The existing test suites for these modules pass unchanged.
