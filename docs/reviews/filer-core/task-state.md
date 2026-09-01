# Current task state review

This report reviews the live `.tasks/` backlog against the current `filer-core`
goal on 2026-06-15. It is review only. It does not change task state.

Evidence comes from `cargo run -q -p taskroot -- validate`, `summary`,
`list --format json`, `ready --domain core --milestone 0.3.0 --format json`,
`ready --format json`, `milestone 0.3.0 --exit-checklist`, and the current
task files under `.tasks/core` and `.tasks/milestones`.

## Snapshot

Validation passes. The inventory is structurally clean.

| Scope | Count | Notes |
| --- | --- | --- |
| Total tasks | 48 | 47 core tasks and 1 milestone |
| Done | 16 | No blocked, deferred, or obsolete tasks are active |
| In Progress | 2 | `CORE-001`, `MILESTONE-003` |
| To Do | 30 | Most contract and audit follow-up work is still open |

Task type mix:

| Type | Count |
| --- | --- |
| Epic | 11 |
| Feature | 7 |
| Bug | 7 |
| Refactor | 10 |
| Design | 5 |
| TestDebt | 4 |
| Docs | 3 |
| Milestone | 1 |

Milestone `0.3.0` contains 15 high-priority items. Three are done,
two are in progress, and ten are still to do. All eight milestone exit
criteria are still open.

## Current state by ambition

The backlog does cover the stated project direction.

| Ambition | Current task coverage | State |
| --- | --- | --- |
| Fast, non-blocking large-directory navigation | `PIPELINE-001` done; `CORE-015`, `CORE-016`, `REL-002`, `MODULES-002`, `CORE-020`, `CORE-021` open | Covered, still gated by correctness and load behavior |
| Reliable public core contracts | `API-001`, `REL-001` done; `PROVIDER-001`, `OPS-001`, `CORE-014`, `CORE-016`, `CORE-024` open | Covered, not stabilized |
| Provider-aware and archive-aware navigation | `VFS-001`, `PROVIDER-001`, `PROVIDER-002`, `NAV-001` | Covered, only the first contract step is in `0.3.0` |
| Semantic extensions with client-neutral output | `MODULES-001`, `MODULES-002`, `PREVIEW-001`, `PROTOCOL-001`, `CORE-003` | Covered, proof path not started |
| File-manager-first programmer support | `MODULES-002`, `CORE-003`, `SEARCH-001`, `PREVIEW-001` | Covered, correctly held behind core contract work |

I do not see a major project ambition with no owner in the current task set.

## Findings

### High

1. The backlog points in the right direction, but the `0.3.0` path is still mostly ahead. Inside the milestone, only `API-001`, `PIPELINE-001`, and `REL-001` are done. The proof tasks for semantic extensions, provider deadlines, segmented locations, undo and conflict contracts, state ownership, reliability coverage, and the three high-severity audit bugs are all still open.

2. The audit remediation backlog lands in the right place. `CORE-014`, `CORE-015`, and `CORE-016` are in milestone `0.3.0`, which matches the verdict that these are the live correctness defects to fix now. The broader hardening work `CORE-017` through `CORE-024` stays outside the milestone, which keeps the release path focused.

3. The task graph does not enforce milestone focus by itself. The ready queue includes the core `0.3.0` tasks that should move the proof target forward, such as `CORE-002`, `MODULES-001`, `OPS-001`, `PROVIDER-001`, `REL-002`, and `VFS-001`. It also includes later or side work such as `NAV-001`, `PIPELINE-002`, `SERVICES-001`, and `SERVICES-002`. Nothing is invalid here, but execution can drift if work selection follows the raw ready list instead of the milestone exit checklist.

### Medium

4. The backlog reads as two layers. The older roadmap tasks mostly date to 2026-06-06. The audit remediation tasks date to 2026-06-13. The two layers agree with each other, but the older tasks have not been refreshed since the audit verdict. That makes the priority story less obvious than it should be when you scan the task list cold.

5. Several future epics are still intentionally broad: `PROTOCOL-001`, `PROVIDER-002`, `PREVIEW-001`, `SEARCH-001`, `OPS-002`, and `PROVIDER-003`. That is acceptable while they stay unmilestoned and inactive. It becomes a real risk only if one of them moves into implementation without first being split into smaller stages.

6. One open high-priority bug sits outside any milestone: `SERVICES-002`. The task itself looks valid and useful, but its scheduling story is unclear from metadata alone. The current backlog makes it ready to execute without saying whether it should compete with `0.3.0` contract work or wait behind it.

### Low

7. The task inventory itself is in good shape. Validation passes, dependency chains are intact, and the milestone hierarchy is consistent. The weakness is not bad metadata.

8. The active core backlog stays inside the project boundary. I do not see open tasks trying to pull `filer-app` or `filer-ecosystem` concerns into `filer-core`.

## Overall verdict

The current tasks steer the project toward the stated `filer-core` ambition.
The backlog covers the right themes, the audit findings were turned into
concrete follow-up work, and the `0.3.0` milestone still captures the core
contract path that the README and roadmap describe.

The weak point is not direction. It is breadth and readability. The milestone
still carries many open gates, and the ready queue exposes later work beside
milestone-critical tasks. That means the backlog is usable as-is, but it still
depends on disciplined task selection. If execution follows the milestone exit
checklist, the project stays on track. If execution follows the broad ready
queue without that filter, drift becomes easy even though the task graph itself
is valid.
