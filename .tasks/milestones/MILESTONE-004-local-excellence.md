---
id: MILESTONE-004
title: Local excellence
status: In Progress
priority: High
type: Milestone
milestone: "0.3.1"
depends_on: [MILESTONE-003]
risk: Medium
impact: "Makes local large-directory browsing and decoration UI feel reliable after core contract stabilization."
tags: [local, excellence, draft]
last_updated: 2026-09-05
---

## Summary

Local file-manager excellence after 0.3.0 contract stabilization. Prove System32-scale browsing and git decoration delivery through public core contracts, measured by a benchmark harness instead of asserted. Core work is CORE-027 audit remediations, PIPELINE-003 scalable page delivery, and CORE-028 benchmarks.

Scope note: every exit criterion here remains a core-side contract gate. app:UI-011 and its children are an approved companion track during 0.3.1, providing real-window feedback without making the app or framework selection a core release gate. App polish and open-in-terminal/editor helpers remain later work.

## Scope change rationale

On 2026-08-26, comparative peer evidence and instrumented whole-pipeline journeys were added through CORE-029 and its children. Internal regression numbers show direction across Filer revisions but cannot show whether Filer is competitive or where a user-visible action stalls. The reference application remains a benchmark client of public Filer-core contracts, so this addition does not reopen filer-app implementation work.

On 2026-08-31, the CORE-004 F24 paging scalability fix moved from the 0.5.0 PIPELINE-002 presentation epic into PIPELINE-003. CORE-028 already makes first-page streaming a 0.3.1 gate, so keeping its implementation in 0.5.0 created a cross-milestone dependency and made this milestone impossible to close in order.

On 2026-09-05, the maintainer approved the backlog review's smaller benchmark release boundary. This milestone requires flat Filer/std/Tokio comparisons, reproducible reports, and one public-core browse journey. CORE-029 remains a milestone-free program; CORE-033, CORE-034, and CORE-042 retain deferred application adapters, frameworks, recursive fixtures, and extended instrumentation. CORE-030 now specifies the design, CORE-039 implements validation and flat fixtures, and CORE-031 stages the runner and reports through CORE-040 and CORE-041.

Later on 2026-09-05, the maintainer approved a bounded app-validation exception because benchmark evidence alone cannot reveal rendering and interaction problems. app:UI-011 stages a framework-free model, public-core bridge, provisional window, and asynchronous decorations through UI-012 to UI-015. The track depends on completed core contracts, not this milestone's completion or the deferred framework evaluation. Its results feed concrete core regression tasks.

## Draft policy

This milestone is a draft plan. You or any agent may modify it as much as needed (exit criteria, membership, priority, depends_on, title, or replacement by a better split) until work for 0.3.1 has started. Work has started when this milestone or any task with `milestone: "0.3.1"` first moves to `In Progress`. Until then, treat this file as editable intent, not a locked commitment. After work starts, change scope only deliberately and record why.

## Candidate membership

- CORE-027 and children CORE-017, CORE-018, CORE-019, CORE-021, CORE-022, CORE-024, PIPELINE-003
- CORE-028 (benchmark harness; gates the performance criteria below)
- CORE-030, CORE-039, CORE-031 with CORE-040/CORE-041, and CORE-032 (initial comparative evidence and one browse journey)
- SERVICES-001 and SERVICES-003 (optional dependency cleanup and a measured detector decision; neither gates this milestone)
- app:UI-011 and children UI-012 through UI-015 (companion validation work, not a core exit gate)
- Reproduced client races use concrete core bug tasks under the workflow in docs/task-tracking.md; REL-006 is retired

## Exit Criteria

Required, not deferrable:

- [x] CORE-017 and CORE-018 are Done: filter/hidden contracts are honest and paging sessions are bounded and documented.
- [ ] CORE-021 is Done: the large-directory hot path sheds its per-row allocation overhead.
- [x] PIPELINE-003 is Done: default large-directory paging emits the first page before end of directory and resumes continuations without a full rewalk while snapshot-only transforms remain correct.
- [x] CORE-028 is Done: the benchmark harness exists, baseline numbers are recorded, and structural tests prove proportional first-page traversal and listing delivery independent of Git completion.
- [ ] CORE-030, CORE-039, CORE-031, and CORE-032 are Done: correctness-checked Filer/std/Tokio baselines, raw reports, and one browse journey are recorded with separate engine, core, and reference-client results.
- [x] MODULES-002 emits semantic row decorations without making listing wait for Git completion, proven by the CORE-028 ordering test and active-decoration comparison.

Deferrable only with recorded rationale in this file:

- [ ] CORE-019, CORE-022, and CORE-024 are Done or explicitly deferred with rationale recorded here.

## Reconciliation evidence

On 2026-09-05, `cargo test -q -p filer-core --test large_directory_paging_test` and `cargo test -q -p filer-core decoration --lib` passed. The baseline in filer-core/benches/baselines/2026-09-04-linux-i7-11800h-btrfs.md records first and next pages, snapshot-only sorting, and active Git overhead. The portable decoration gate proves independent event delivery; it does not promise zero CPU contention. CORE-017, CORE-018, PIPELINE-003, and CORE-028 have completed task criteria and committed implementation evidence.
