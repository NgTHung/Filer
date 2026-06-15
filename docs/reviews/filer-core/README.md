# filer-core review reports

This directory holds the filer-core architecture and code-quality audit reports from
epic `CORE-004`, plus later review-only backlog reports. These documents do not change
production code. Concrete fixes stay in tracked follow-up tasks.

## Convention

One markdown report per review pass. Each report ranks findings by severity and lists
follow-up task candidates so remediation can be triaged by priority.

| Report | Task | Scope |
| --- | --- | --- |
| `architecture-fit.md` | CORE-005 | Can the Location, actor/cancellation, extension, and transport designs carry the ambitions |
| `module-size.md` | CORE-006 | Files over the 700/1000 LoC limits and proposed split boundaries |
| `rust-rules.md` | CORE-007 | unwrap/expect, clones, `Result + ?`, silent error swallowing |
| `async-actors.md` | CORE-008 | Cancellation, stale-result guards, task leaks, channel backpressure, shutdown |
| `vfs-provider.md` | CORE-009 | FsProvider surface, paging, capabilities, timeout/segmented routing readiness |
| `model-pipeline.md` | CORE-010 | Location/Node/Query types, pipeline transforms, cursor stability |
| `test-suite.md` | CORE-011 | Coverage by subsystem, fixture patterns, missing cancellation/timeout tests |
| `documentation.md` | CORE-012 | Comment-rule compliance, README/DESIGN accuracy vs code |
| `VERDICT.md` | CORE-013 | Consolidated verdict and prioritized remediation backlog |
| `task-state.md` | review only | Current `.tasks/` state against filer-core ambitions and milestone `0.3.0` |

## Severity scale

Each finding uses one of: **Critical** (blocks an ambition or forces a rewrite),
**High** (significant rework cost if deferred), **Medium** (maintainability or correctness
risk), **Low** (cleanup). Follow-up tasks inherit priority from severity.
