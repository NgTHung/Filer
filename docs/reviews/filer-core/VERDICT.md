# filer-core Audit Verdict (CORE-013)

Status note, 2026-06-24: this verdict is historical. The incomplete feature-gated S3,
WebDAV, FTP/SFTP, FUSE, Kubernetes, and `RemoteProvider` stubs referenced below were removed.
`PROVIDER-002` now tracks provider registry and VFS contract stabilization before concrete
remote or mount providers return.

This rolls up the eight review passes of epic CORE-004 into one decision and one prioritized
remediation backlog. It answers a single question: does the current filer-core design support
the project's stated ambitions, or must it be reworked before more features land? It then
consolidates and de-duplicates every finding from the eight reports and points each accepted
finding at the task that will fix it.

The eight source reports are `architecture-fit.md` (CORE-005), `module-size.md` (CORE-006),
`rust-rules.md` (CORE-007), `async-actors.md` (CORE-008), `vfs-provider.md` (CORE-009),
`model-pipeline.md` (CORE-010), `test-suite.md` (CORE-011), and `documentation.md` (CORE-012).

## Verdict

The architecture stands. It supports the ambitions and does not need a rewrite. Close one
correctness bug and a small cluster of contract defects before the features that depend on them
land.

The load-bearing decisions are sound. The Location addressing model separates identity from
display and is serializable end to end, so it is already transport-ready and represents
unsupported routes instead of assuming them away. Long work is actor-isolated with per-session
cancellation. Directory transformation flows through one pipeline contract. The provider trait
models capability-limited and read-only providers honestly. None of these will collapse under
load.

The risk is not structural rot. It is two distinct things. First, a set of deferred contracts
on wide call surfaces (the wire-safe extension envelope, the provider deadline context, provider
resolution, the remote-provider lifecycle). Each of those is already owned by a roadmap task, and
the audit's recommendation is about sequencing: define each contract before the first feature
that would otherwise harden the wrong shape around it. Second, a thin layer of correctness defects
that exist in the code today and are owned by no task: a panic on valid input, two listing-order
authorities that disagree, a cancellation-cleanup race, and several pipeline contracts that accept
configuration they never honor. These are the new work this verdict creates.

The single must-fix-now item is the `NodeId::from_path` panic on non-UTF-8 paths. It sits on the
main scan path, triggers on input that is legal on both target platforms, and contradicts the
reliability priority directly. Everything else is scheduling and hardening.

## Ambition scorecard

| Ambition | Supported today | Gating work |
| --- | --- | --- |
| Fast non-blocking navigation of very large directories | Yes, with caveats | Cancellable fallback scan loop, shutdown quiesce, event backpressure, pipeline hot-path allocation (CORE-016, CORE-020, CORE-021) |
| Cross-client core (desktop now, web/server later) | Yes, model is transport-ready | Wire-safe extension envelope (MODULES-001), versioned transport (PROTOCOL-001) |
| Pluggable providers without core churn | Structurally yes | Deadline context (PROVIDER-001), provider resolution and remote lifecycle (PROVIDER-002), segment routing (VFS-001) |
| Semantic extensions emitting data across a boundary | Not yet | Serializable extension data plane (MODULES-001) |
| Reliability: cancellation, stale suppression, structured errors | Mostly | NodeId panic (CORE-014), cancellation clobber (CORE-016), dispatcher diagnostics (CORE-024) |

## Consolidated findings

Findings are de-duplicated across the eight passes and ranked by severity. The "Disposition"
column names the task that owns the fix. "Owned" means an existing roadmap task already covers the
outcome and no new task is created. "New" names a task created by this synthesis.

### Critical and High

| # | Finding | Sources | Severity | Disposition |
| --- | --- | --- | --- | --- |
| F1 | `NodeId::from_path` calls `to_str().unwrap()` and panics on non-UTF-8 paths, on the main scan path | CORE-007, CORE-010 | High | New: CORE-014 |
| F2 | Listing order is defined twice (`SortBy` vs `compare_nodes`) and the two disagree on tie-breaking and extension direction; cursor stability rests on the comparator the snapshot path does not use | CORE-010 | High | New: CORE-015 |
| F3 | Searcher and previewer use unconditional `remove` instead of `remove_if_current`, so rapid re-issue orphans an uncancellable task; the cancellation primitive `CancelMap` has no direct unit test | CORE-008, CORE-011 | High | New: CORE-016 |
| F4 | Extension payload is in-process `Arc<dyn Any>` only; no serializable envelope crosses a boundary | CORE-005 | Critical | Owned: MODULES-001, PROTOCOL-001 |
| F5 | No deadline or cancellation context in any `FsProvider` method; the token stops at the provider boundary. Concrete `ProviderCx` shape produced in CORE-009 | CORE-005, CORE-009 | High | Owned: PROVIDER-001 |
| F6 | `RemoteProvider` declares `&mut self` lifecycle methods that are unreachable through the `Arc<dyn FsProvider>` wiring; the first remote provider would not compile | CORE-009 | High | Owned: PROVIDER-002 |
| F7 | No provider registry or scheme-to-provider resolution; the system is hardwired to one `LocalFs` | CORE-009 | High | Owned: PROVIDER-002 |
| F8 | `operator.rs` (1431) and `scanner.rs` (994, one 530-line function) breach the size limits at real cohesion seams | CORE-006 | High | New: CORE-019 |

### Medium

| # | Finding | Sources | Severity | Disposition |
| --- | --- | --- | --- | --- |
| F9 | `FilterConfig` size and name filters are accepted, force `SnapshotOnly` and full-metadata listing, then never filter | CORE-010 | Medium | New: CORE-017 |
| F10 | Pipeline `FilterHidden` recomputes hidden state from the path and is wrong on Windows, diverging from the platform-correct `meta.hidden` that the search filter uses | CORE-010 | Medium | New: CORE-017 |
| F11 | Group display order keys on the display string and ignores `TimeGroup`/`SizeGroup::sort_order`, so date and size groups render in alphabetical, not logical, order | CORE-010 | Medium | New: CORE-015 |
| F12 | The paging session map has no TTL or cap and grows with abandoned pagination; the cursor is single-use and its stale-key duplicate/skip behavior under mutation is undocumented | CORE-010 | Medium | New: CORE-018 |
| F13 | Detached per-command tasks are not joined on shutdown, so `FilerCore::shutdown` can return while a copy or scan still touches the filesystem | CORE-008 | Medium | New: CORE-020 |
| F14 | The fallback paging path runs `PageSelection::extend` over the whole directory with no cancellation check, so a cancel is not observed until the loop finishes | CORE-008, CORE-009 | Medium | New: CORE-020 |
| F15 | Every channel is unbounded; high-volume event streams (scan pages, per-file copy progress, watcher changes) can grow without bound against a slow consumer | CORE-008 | Medium | New: CORE-020 |
| F16 | Command-dispatcher sends use `let _ = tx.send(...)` and silently drop commands on a closed channel, bypassing the crate's own `send_or_warn` helper | CORE-007 | Medium | New: CORE-024 |
| F17 | Grouped pipeline stages clone every node, and `PageSelection::extend` runs the full pipeline per row on the hot path | CORE-010 | Medium | New: CORE-021 |
| F18 | Preview rendering bypasses the provider and opens local paths directly, so previews are local-only | CORE-009 | Medium (High once remote reachable) | Owned: PREVIEW-001 |
| F19 | The `write` boolean is too coarse for per-operation capability, and capability is split across three mechanisms | CORE-009 | Medium | Owned: PROVIDER-002 |
| F20 | Test fixtures (`make_file` in four conflicting shapes), mock providers (five copies), and `build_core` harnesses are duplicated across the suite with no shared support module | CORE-011 | Medium | New: CORE-022 |
| F21 | Async actor tests synchronize with fixed `sleep` plus `timeout` races, making the watcher and navigator suites load-sensitive and slow | CORE-011 | Medium | New: CORE-022 |
| F22 | `cancel.rs` (`CancelMap`) has no direct unit test, the riskiest and least-covered code in the crate | CORE-011 | High (coverage) | New: CORE-016 |
| F23 | A command-rename sweep corrupted README prose, replacing verbs like "search" with `SearchNodeCompat`; the `filer-core/README.md` Modules table lists a non-existent `bus/` and misplaces the workers; `vfs/local.rs:214` carries a stale `# TODO` rustdoc section on an implemented method | CORE-012 | Medium | New: CORE-023 |
| F24 | The native and keyset cursors do not compose, so a "next page" rewalks the whole directory; O(directory) work per page on the large-directory target | CORE-009, CORE-010 | Medium | Owned: PIPELINE-003 |

### Low

These are real but minor. Most are trivial enough to fix opportunistically when the file is next
touched and do not warrant their own task. They are recorded here so they are not lost.

- `code.rs:61` theme fallback ends in `unwrap` on an undocumented library invariant; prefer an
  unstyled fallback. Folded into CORE-024.
- Feature-gated remote providers used `todo!()` bodies. The stubs were removed on 2026-06-24,
  and CORE-024 now records that cleanup.
- `operations/mod.rs:61` panic message names the wrong module ("ScanModule"). Folded into CORE-024.
- `navigator.rs` split (state machine vs actor, clean seam) and `EXT_TABLE` compression. The split
  is folded into CORE-019. The table compression is deferred pending SERVICES-003, which may
  replace the hand-maintained table with the file-type crate; compressing it now risks wasted work.
- `NameMatches` recompiles its regex per node; carry the compiled `Regex`. Opportunistic with
  SEARCH-001.
- `GroupBy::Type` silently means "by extension"; `FileNode`/`NodeEntry` near-duplication;
  `model/node.rs` WHAT-restating comments; dead stub `src/tests/modules/navigation_flow_test.rs`;
  empty `docs/README.md`; module-doc backfill for high-traffic files. Opportunistic cleanup, no
  task.
- The eight `Mutex::lock().unwrap()` sites are the standard poison-only idiom and are accepted. A
  non-poisoning mutex would remove them but is a dependency decision, not a fix.

## Decisions this synthesis makes

Two findings asked the synthesis pass to make a call rather than defer.

Backpressure (F15). Adopt bounded channels with a defined overflow policy for the high-volume
event streams only: scan page and progress events, per-file operation progress, and the watcher
change feed. Command channels stay unbounded because commands are small and drained promptly.
Progress-style events should coalesce on overflow (keep the latest, drop intermediate) rather than
block the producer, so a slow consumer cannot stall a scan. Page and change events should block the
producer to preserve completeness. Implementation lands in CORE-020.

Markdown in doc comments (CORE-012, F23 neighbor). The AGENTS.md ban on markdown in comments
conflicts with rustdoc, where markdown is the native rendering format. Recommendation, pending user
sign-off: carve an explicit exception in AGENTS.md for rustdoc-rendered doc comments (`///` and
`//!`), and keep the hard no-markdown rule for plain `//` inline comments. This is a rule change, so
it stays a recommendation here rather than an edit.

Cursor rewalk (F24). This is a scalability ceiling, not a correctness bug, and the fix spans the
pipeline, the provider cursor, and the paging session. It became PIPELINE-003, staged as
PIPELINE-004 (a resumable provider walk), PIPELINE-005 (page assembly chosen by pipeline paging
mode), and PIPELINE-006 (retained ordered continuations), rather than folding into the
presentation epic.

## Remediation backlog

New tasks created by this synthesis, in priority order. None duplicates an existing roadmap task;
the architectural findings (F4 through F7, F18, F19) are left to their owners and referenced above.

| Task | Title | Severity | Fixes |
| --- | --- | --- | --- |
| CORE-014 | Fix NodeId hashing panic on non-UTF-8 paths | High | F1 |
| CORE-015 | Unify directory listing and group ordering on one comparator | High | F2, F11 |
| CORE-016 | Fix cancellation cleanup clobber and unit-test CancelMap | High | F3, F22 |
| CORE-017 | Honor or remove the pipeline filter and hidden-file contracts | Medium | F9, F10 |
| CORE-018 | Bound paging session lifetime and document the cursor contract | Medium | F12 |
| CORE-019 | Decompose the oversized operator, scanner, and navigator modules | Medium | F8, navigator split |
| CORE-020 | Harden actor cancellation, shutdown, and event backpressure | Medium | F13, F14, F15 |
| CORE-021 | Cut pipeline hot-path allocation on the large-directory scan | Medium | F17 |
| CORE-022 | Consolidate test fixtures and stabilize async test synchronization | Medium | F20, F21 |
| CORE-023 | Repair filer-core README prose and stale rustdoc | Medium | F23 |
| CORE-024 | Restore diagnostic error handling in dispatchers and providers | Medium | F16, low-severity error-handling items |

Suggested sequencing. CORE-014 first; it is the only live panic on valid input. CORE-015 next,
because cursor stability (CORE-018) depends on a single ordering authority. CORE-016 alongside,
because the cancellation clobber is the one async correctness defect and its unit test is the
crate's largest coverage gap. The High-severity correctness tasks (CORE-014, CORE-015, CORE-016)
are 0.3.0 candidates; the rest are quality and hardening that can follow.
