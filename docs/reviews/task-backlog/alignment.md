# Task Backlog Alignment Review

This review answers one question. Does the current `.tasks/` backlog steer filer-core
toward the project's stated ambitions, or does it drift, leave gaps, or harden the wrong
shape? It is review-only. It creates no tasks and edits no task files. Findings carry a
recommended disposition for a later refinement pass.

The backlog holds 48 tasks. Structural consistency (parents resolve, milestones match,
dependencies form a DAG, rule IDs exist, Done tasks have checked criteria) is already
guaranteed by `taskroot validate`. This review covers semantic alignment instead.

## Ambitions under test

From `ROADMAP.md`, `docs/architecture/invariants.md`, and
`docs/reviews/filer-core/architecture-fit.md`:

1. Fast, non-blocking navigation of very large local directories.
2. Cross-client core: one core behind desktop now, web and server later, identical behavior.
3. Pluggable providers without core churn.
4. Semantic extensions emitting data, not UI, across a transport boundary.
5. Reliability: cancellation, stale-result suppression, structured errors, freshness under mutation.

Plus the phase proof target from `ROADMAP.md`: load a very large local directory such as
`C:\Windows\System32` without blocking the client, then apply git-style decorations
asynchronously in a large repository without blocking the load.

Plus the product boundary: file manager first, not an IDE. Programmer features stay helpful
reading tools.

## Verdict

The backlog steers toward the ambitions. Every ambition and both halves of the proof target
have at least one owning task, and the contract-before-feature sequencing the architecture
review demanded is mostly encoded in dependency edges. No task drifts into IDE territory.
The problems are not coverage or direction. They are three smaller things: a cluster of work
that does real reliability work but carries no alignment metadata, one missing dependency
edge that lets a provider feature outrun its contract, and an ambiguity about whether the
proof target is a 0.3.0 exit gate. None requires reworking the backlog. Each is a targeted
refinement.

## Ambition coverage matrix

| Ambition | Owning tasks | Coverage |
| --- | --- | --- |
| 1. Non-blocking navigation of large dirs | PIPELINE-001 (done), CORE-020, CORE-021, NAV-001, MODULES-002 | Covered. Gating perf work (CORE-020, CORE-021) sits outside 0.3.0. See F3. |
| 2. Cross-client core | API-001 (done), MODULES-001, PROTOCOL-001 | Covered. Contract first (API-001 done, envelope in MODULES-001), transport deferred to PROTOCOL-001. Correct order. |
| 3. Pluggable providers | PROVIDER-001, PROVIDER-002, PROVIDER-003, VFS-001, SEARCH-001 | Covered. One sequencing edge missing. See F2. |
| 4. Semantic extensions | MODULES-001, MODULES-002, CORE-003 | Covered. MODULES-001 is the gating contract; both consumers depend on it. |
| 5. Reliability | REL-001 (done), REL-002, CORE-014, CORE-015, CORE-016, CORE-020, CORE-024, SERVICES-002 | Covered, strongest area. SERVICES-002 is reliability work outside the graph. See F1. |
| Proof target (System32 load) | PIPELINE-001 (done), CORE-020, CORE-021 | Half-covered in 0.3.0. Perf hardening deferred. See F3. |
| Proof target (async git decorations) | MODULES-002 | Covered, in 0.3.0, with a large-repo test. |
| Product boundary (not an IDE) | CORE-003 watch item | Respected. See F7. |

Reverse check against MILESTONE-003 exit criteria: every exit criterion has an owning task.
Compatibility names (API-001, done), provider timeout context (PROVIDER-001), cross-provider
paging (PIPELINE-001, done), segmented Location (VFS-001), undo and conflict contracts
(OPS-001), git decoration prototype (MODULES-002), state ownership (CORE-002), the three
high-severity correctness fixes (CORE-014, CORE-015, CORE-016). No exit criterion is orphaned.

## Per-task alignment

Verdict legend: Aligned (direction and metadata correct), Metadata gap (right work, missing
or wrong alignment fields), Watch (correct today, revisit at implementation), Sequencing
(direction correct, dependency or milestone needs attention).

### 0.3.0 contract work (parent CORE-001 / MILESTONE-003)

| ID | Status | Ambition | Verdict |
| --- | --- | --- | --- |
| CORE-001 | In Progress | All (epic) | Aligned. Epic container for 0.3.0 contracts. |
| CORE-002 | To Do | 2, state boundary | Aligned. |
| API-001 | Done | 2 | Aligned. Unversioned DTO is the intended represent-first shape; PROTOCOL-001 versions it later. |
| REL-001 | Done | 5 | Aligned. Landed before MODULES-001 and PROVIDER-001 that depend on it. Correct order. |
| REL-002 | To Do | 5 | Aligned. Confirm test boundary vs CORE-016/CORE-020. See F6. |
| PIPELINE-001 | Done | 1, 3 | Aligned. Proves cross-provider paging half of the proof target. |
| MODULES-001 | To Do | 2, 4 | Aligned. The gating extension envelope contract. |
| MODULES-002 | To Do | 1, 4 | Aligned. Proves the async git-decoration half of the proof target. |
| PROVIDER-001 | To Do | 1, 3 | Aligned. The deadline contract R2 requires before remote providers. |
| VFS-001 | To Do | 3 | Aligned. Segment routing per R3. |
| OPS-001 | To Do | file ops | Aligned. Defines undo and conflict contracts before OPS-002 builds on them. |

### Audit review tasks (epic CORE-004, all Done)

CORE-004 through CORE-013. These produced `architecture-fit.md` and `VERDICT.md` and created
the remediation backlog. Aligned: they directly answered "does the architecture carry the
ambitions." CORE-004 status is discussed in F5.

### Remediation backlog (parent CORE-004)

| ID | Status | Milestone | Ambition | Verdict |
| --- | --- | --- | --- | --- |
| CORE-014 | To Do | 0.3.0 | 5 | Aligned. The one live panic on valid input. |
| CORE-015 | To Do | 0.3.0 | 1, 5 | Aligned. Single ordering authority, gates cursor stability. |
| CORE-016 | To Do | 0.3.0 | 5 | Aligned. Cancellation clobber fix plus CancelMap test. |
| CORE-017 | To Do | none | 1, 5 | Aligned. Pipeline filter and hidden-file correctness. |
| CORE-018 | To Do | none | 1 | Aligned. Bounds paging session lifetime. |
| CORE-019 | To Do | none | maintainability | Aligned. Decomposes oversized modules at real seams. |
| CORE-020 | To Do | none | 1, 5 | Sequencing. Gates the proof target but is not in 0.3.0. See F3. |
| CORE-021 | To Do | none | 1 | Sequencing. Same as CORE-020. See F3. |
| CORE-022 | To Do | none | 5, tests | Aligned. |
| CORE-023 | To Do | none | docs | Aligned. |
| CORE-024 | To Do | none | 5 | Aligned. Restores diagnostic error handling. |

### Future epics (unmilestoned)

| ID | Status | Ambition | Verdict |
| --- | --- | --- | --- |
| NAV-001 | To Do | 1 | Aligned. Frontend-independent navigation and session restore. |
| OPS-002 | To Do | file ops | Aligned. Depends on OPS-001 contract. Correct order. |
| PIPELINE-002 | To Do | presentation | Aligned. Owns presentation only; VERDICT F24 moved to PIPELINE-003. |
| PREVIEW-001 | To Do | 3 | Aligned. Removes local-path assumption from previews. |
| PROTOCOL-001 | To Do | 2 | Aligned. Depends on API-001 and MODULES-001. Correctly post-0.3.0. |
| PROVIDER-002 | To Do | 3 | Sequencing. Missing PROVIDER-001 dependency. See F2. |
| PROVIDER-003 | To Do | 3 | Aligned. |
| SEARCH-001 | To Do | 3 | Aligned. |
| CORE-003 | To Do | 4 | Watch. IDE-drift boundary. See F7. |

### Services and tooling

| ID | Status | Ambition | Verdict |
| --- | --- | --- | --- |
| SERVICES-001 | To Do | 5 | Metadata gap. No rules, no parent, no milestone. See F1. |
| SERVICES-002 | To Do | 5 | Metadata gap. Correctness bug, provider-boundary, no metadata. See F1. |
| SERVICES-003 | To Do | 5, perf | Metadata gap. See F1. |
| UTILS-001 | Done | governance | Aligned. Task-driven workflow. |
| UTILS-002 | Done | governance | Aligned. |
| UTILS-003 | Done | governance | Aligned. |

## Findings

Ranked by how much each affects steering. Each lists a recommended disposition. No task is
created or edited by this review.

### F1 — Services work has no alignment metadata (Medium-High)

SERVICES-001, SERVICES-002, and SERVICES-003 carry no `rules`, no `parent`, and no
`milestone`. They do real reliability work. SERVICES-002 is a High-priority correctness bug:
`LocalFs::read_header` uses `read_exact`, so files smaller than the window silently get no
magic-byte detection. That sits on the provider read boundary and contradicts the reliability
priority, the same class of defect as the CORE-014 audit fixes, yet it lives outside the
ambition graph entirely.

Disposition: declare rules (SERVICES-002 touches the provider boundary, so PROVIDER-ACCESS
applies; SERVICES-001 and SERVICES-003 are internal type-detection services and may legitimately
carry none). Parent the three under a reliability or services epic so they trace to an ambition.
Reassess SERVICES-002 for 0.3.0 inclusion next to CORE-014/015/016, since it is the same kind of
silent correctness defect. Encode the EXT_TABLE link the VERDICT notes between SERVICES-003 and
CORE-019 as a relationship.

### F2 — PROVIDER-002 can outrun its deadline contract (Medium)

PROVIDER-002 (expand providers) depends on VFS-001 and CORE-002, but not on PROVIDER-001.
The architecture review R2 and VERDICT F5 both state the provider deadline and cancellation
context must land before the second provider ships, or remote providers get retrofitted across
a wide call surface and the non-blocking ambition breaks the moment a slow remote provider is
wired in. The dependency edge that would enforce this is missing.

Disposition: add `PROVIDER-001` to PROVIDER-002's `depends_on`.

### F3 — Proof-target performance hardening sits outside 0.3.0 (Medium)

`ROADMAP.md` frames the System32 non-blocking load as the proof target for this phase. The
VERDICT scorecard names CORE-020 (cancellable fallback scan, shutdown quiesce, event
backpressure) and CORE-021 (pipeline hot-path allocation) as the work gating that ambition.
Both are unmilestoned. MILESTONE-003 exit criteria cover only the three correctness fixes
(CORE-014/015/016), not the perf work. So the metadata says the proof target is not a 0.3.0
gate, which reads against the roadmap framing.

Disposition: decide explicitly. If the System32 proof is a 0.3.0 exit gate, move CORE-020 and
CORE-021 into 0.3.0 and add a proof exit criterion. If the proof is a later-phase goal, state
that in the milestone so the framing and the metadata agree. This is a scoping decision for the
user, not a metadata typo.

### F4 — No single task asserts the whole proof target (Low-Medium)

PIPELINE-001 proves cross-provider paging. MODULES-002 proves async git decorations with a
large-repo test. Neither asserts the combined headline: load a very large directory without
blocking and apply git decorations asynchronously without blocking the load. The proof target
is the phase's headline but no task verifies it end to end.

Disposition: consider a verification task or a milestone exit criterion that demonstrates the
combined proof, rather than trusting two half-proofs to compose.

### F5 — CORE-004 is Done but owns eleven open children (Low)

CORE-004 (audit epic) is Done. It parents CORE-014 through CORE-024, all To Do, three of them
0.3.0 deliverables. The Done status means the audit finished, but the epic now reads complete
while its remediation children are open. Validation permits this and milestone tracking is
unaffected, because the three 0.3.0 fixes carry their own milestone field.

Disposition: optional. Either move CORE-014..024 under a short-lived remediation epic that
tracks rollup status, or accept the convention that an audit epic completes when the audit
completes and document it. Low impact.

### F6 — Cancellation test ownership overlaps (Low)

REL-002 (reliability coverage gaps) lists cancellation tests and watcher burst ordering.
CORE-016 adds the CancelMap unit test and CORE-020 hardens cancellation and shutdown. The
boundary between these is not explicit, so cancellation coverage could be written twice or
fall between them.

Disposition: confirm the test ownership split in REL-002 and CORE-016/CORE-020, no structural
change needed.

### F7 — CORE-003 stays on the right side of the IDE boundary (informational)

CORE-003 (programmer helper contracts) is the obvious IDE-drift risk. Reviewed against the
"Programmer Features Are Helpful Reading Tools" invariant: its criteria are terminal and editor
launchers, repo detection, and git, converter, and syntax helpers as extensions. No continuous
compilation, no debugging, no language-server intelligence. It is priority Low and gated behind
MODULES-001. Aligned today.

Disposition: no change. Re-check scope at implementation time, since this is where drift would
enter if it ever did.

### F8 — Future epics have no milestone to attach to (informational)

NAV-001, OPS-002, PIPELINE-002, PREVIEW-001, PROTOCOL-001, PROVIDER-002, PROVIDER-003,
SEARCH-001, and CORE-003 are unmilestoned. The roadmap defines milestone labels (Power,
Protocol, Ecosystem, Future) but only MILESTONE-003 exists as a milestone task, so these epics
have nothing to attach to. Their ordering lives only in `depends_on` edges. This is acceptable
for a backlog. Cross-milestone sequencing is just invisible to milestone queries.

Disposition: optional. Define future milestone tasks, or accept that the backlog past 0.3.0 is
dependency-ordered rather than milestone-ordered.

## What is well aligned

Do not disturb these. They are the reason the verdict is positive.

- Contract-before-feature is encoded where it matters most. MODULES-002 depends on MODULES-001.
  OPS-002 depends on OPS-001. PROTOCOL-001 depends on API-001 and MODULES-001. REL-001 landed
  before its dependents. API-001's unversioned DTO is the deliberate represent-first shape.
- Every ambition and both halves of the proof target have an owning task.
- Every MILESTONE-003 exit criterion has an owning task.
- The remediation backlog points each accepted audit finding at exactly one task, with no
  duplication against the roadmap features.
- No task drifts toward IDE scope.

## Recommended refinement order

If a refinement pass follows, address findings in this order. Sequencing and metadata first,
since they change what the backlog says about itself, then the scoping decisions.

1. F2: add the PROVIDER-001 edge to PROVIDER-002 (one field, removes a real sequencing risk).
2. F1: give the SERVICES tasks rules, a parent, and a milestone decision.
3. F3: decide whether the proof target gates 0.3.0, then align CORE-020/CORE-021 and the
   milestone accordingly.
4. F4, F6: clarify proof-target verification and cancellation test ownership.
5. F5, F8: optional structural tidying of epic rollup and future milestones.

None of these is an implementation task. Each is a backlog refinement. Findings that turn out
to need code, such as the SERVICES-002 fix, already have their own tasks.
