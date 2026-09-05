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

Scope note: AGENTS.md pins repository focus to filer-core, so every exit criterion here is a core-side contract gate. filer-app consumption of these contracts is the intended demo, not a gate; app polish and open-in-terminal/editor helpers move to 0.4.0 (CORE-003) or dedicated app tasks filed later.

## Scope change rationale

On 2026-08-26, comparative peer evidence and instrumented whole-pipeline journeys were added through CORE-029 and its children. Internal regression numbers show direction across Filer revisions but cannot show whether Filer is competitive or where a user-visible action stalls. The reference application remains a benchmark client of public Filer-core contracts, so this addition does not reopen filer-app implementation work.

On 2026-08-31, the CORE-004 F24 paging scalability fix moved from the 0.5.0 PIPELINE-002 presentation epic into PIPELINE-003. CORE-028 already makes first-page streaming a 0.3.1 gate, so keeping its implementation in 0.5.0 created a cross-milestone dependency and made this milestone impossible to close in order.

## Draft policy

This milestone is a draft plan. You or any agent may modify it as much as needed (exit criteria, membership, priority, depends_on, title, or replacement by a better split) until work for 0.3.1 has started. Work has started when this milestone or any task with `milestone: "0.3.1"` first moves to `In Progress`. Until then, treat this file as editable intent, not a locked commitment. After work starts, change scope only deliberately and record why.

## Candidate membership

- CORE-027 and children CORE-017, CORE-018, CORE-019, CORE-021, CORE-022, CORE-024, PIPELINE-003
- CORE-028 (benchmark harness; gates the performance criteria below)
- CORE-029 and children CORE-030 through CORE-034 (comparative protocol, peer adapters, whole-pipeline driver, and reports)
- SERVICES-001 and SERVICES-003 (dependency cleanup; SERVICES-003 may close without the swap per its own criteria)
- REL-006 stays milestone-free by its recorded rationale; reproduced races landing during 0.3.1 still route through it

## Exit Criteria

Required, not deferrable:

- [x] CORE-017 and CORE-018 are Done: filter/hidden contracts are honest and paging sessions are bounded and documented.
- [ ] CORE-021 is Done: the large-directory hot path sheds its per-row allocation overhead.
- [x] PIPELINE-003 is Done: default large-directory paging emits the first page before end of directory and resumes continuations without a full rewalk while snapshot-only transforms remain correct.
- [x] CORE-028 is Done: the benchmark harness exists, baseline numbers are recorded, and structural tests prove proportional first-page traversal and listing delivery independent of Git completion.
- [ ] CORE-029 is Done: correctness-checked peer baselines and whole-pipeline browse, presentation, search, cancellation, and responsiveness journeys are recorded without mixing in-process and application leaderboards.
- [x] MODULES-002 emits semantic row decorations without making listing wait for Git completion, proven by the CORE-028 ordering test and active-decoration comparison.

Deferrable only with recorded rationale in this file:

- [ ] CORE-019, CORE-022, and CORE-024 are Done or explicitly deferred with rationale recorded here.

## Reconciliation evidence

On 2026-09-05, `cargo test -q -p filer-core --test large_directory_paging_test` and `cargo test -q -p filer-core decoration --lib` passed. The baseline in filer-core/benches/baselines/2026-09-04-linux-i7-11800h-btrfs.md records first and next pages, snapshot-only sorting, and active Git overhead. The portable decoration gate proves independent event delivery; it does not promise zero CPU contention. CORE-017, CORE-018, PIPELINE-003, and CORE-028 have completed task criteria and committed implementation evidence.
