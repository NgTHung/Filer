# Test-Suite Quality and Coverage Review (CORE-011)

Does the filer-core test suite, for a project that calls itself TDD, actually cover the
subsystems it ships, and are the tests written consistently enough to trust and maintain? This
report inventories coverage by subsystem, flags the gaps, documents fixture and test-style
inconsistencies with examples, and lists test-debt follow-ups. It is review-only and changes no
test or production code.

Evidence is cited as `path:line` against the crate at the time of review. Counts come from
`cargo test -p filer-core` and from the test sources under `filer-core/src/tests/` (unit suite)
and `filer-core/tests/` (integration suite). REL-002 already owns the reliability-focused
coverage gaps; this report points at it rather than restating its criteria.

## Headline numbers

The suite is green and substantial. `cargo test -p filer-core` reports 742 tests passing across
six binaries, with 21 ignored:

| Suite | Passed | Ignored |
| --- | --- | --- |
| lib unit tests (`src/tests/`) | 712 | 0 |
| `tests/navigation_flow_test.rs` | 17 | 0 |
| `tests/scanner_integration_test.rs` | 7 | 0 |
| `tests/search_integration_test.rs` | 3 | 0 |
| `tests/stress_test.rs` | 0 | 10 |
| doctests | 3 | 11 |

Test code outweighs production code. Production source (excluding `src/tests/`) is about 17,500
LoC; in-crate test modules are about 19,700 LoC and the external integration suite adds about
2,550 more, for a test-to-code ratio near 1.3:1. The raw ratio is healthy. The problem is not
volume, it is distribution and consistency, covered below.

## Coverage by subsystem

The mapping below pairs each production subsystem with the test modules that exercise it and
rates direct coverage. "Direct" means a test module instantiates the type under test; "indirect"
means the code is only reached through a higher-level path (the public API or an actor) without a
focused test.

| Subsystem | Production | Test modules | Direct coverage |
| --- | --- | --- | --- |
| model (Location, Node, Query, Directory, Session, Capability) | `model/` 2,109 | `tests/model/*` (8 files, ~150 tests) | Strong |
| pipeline (transforms, paging selection, cursors) | `pipeline/` 858 | `tests/pipeline/pipeline_test.rs` (66) | Strong |
| vfs / FsProvider | `vfs/` 1,516 | `tests/vfs/vfs_test.rs` (45) | Strong |
| mime detection + table | `services/mime/` 1,390 | `tests/infra/mime_test.rs` (47), `table_test.rs` (14) | Strong |
| metadata extraction | `services/metadata/` ~1,200 | `tests/infra/metadata_test.rs` (51) | Moderate (see gaps) |
| preview registry + cache | `services/preview/` ~1,000 | `tests/infra/preview_test.rs` (13) | Registry/cache only |
| dir cache | `services/dir_cache.rs` 226 | `tests/infra/dir_cache_test.rs` (20) | Strong |
| errors | `errors.rs` 482 | `tests/infra/error_test.rs` (16) | Strong |
| scanner actor + paging | `modules/scan/` 1,524 | `tests/modules/scanner_test.rs` (56), `tests/scanner_integration_test.rs` (7) | Strong |
| operator actor | `modules/operations/operator.rs` 1,431 | `tests/modules/operator_test.rs` (35) | Strong |
| searcher actor | `modules/search/` 451 | `tests/modules/search_test.rs` (40), `tests/search_integration_test.rs` (3) | Strong |
| navigator actor | `modules/navigation/navigator.rs` 707 | `tests/modules/navigator_test.rs` (38), `tests/navigation_flow_test.rs` (17) | Strong |
| previewer actor | `modules/preview/previewer.rs` 664 | `tests/modules/previewer_test.rs` (9) | Thin (see gaps) |
| watcher actor | `modules/watch/watcher.rs` 371 | `tests/modules/watcher_test.rs` (14) | Moderate, timing-fragile |
| api handle + router | `api/` 1,797 | `tests/api/*` (3 files, 69 tests) | Strong |
| actor infra (system, router) | `actors/mod.rs`, `actors/router.rs` | `tests/infra/actor_test.rs` (6), via `command_router_test` | Indirect |
| **cancellation core** | `actors/cancel.rs` 126 | none | **None (gap)** |

## Coverage gaps, ranked

### The cancellation primitive has no direct test (High)

`actors/cancel.rs` defines `CancelMap` with `arm`, `is_latest`, `remove`, and `remove_if_current`
(`cancel.rs:68`, `:82`, `:99`, `:104`). No test module instantiates `CancelMap` or calls
`remove_if_current` by name; a grep for those symbols across `src/tests/` and `tests/` returns
nothing. This primitive is the single most defect-prone piece in the crate. CORE-008 identified
the `remove` vs `remove_if_current` clobber as the central async correctness bug, and the unit
that would catch it has zero direct coverage. Every cancellation guarantee is currently tested
only indirectly, through actor-level tests that race on timing (next section). A focused unit test
of the `arm` then stale-`remove` interleaving would pin the contract deterministically. Severity
High because it is both the riskiest code and the least directly tested, and it couples to an
already-identified live bug.

### Preview providers are untested individually (Medium)

`preview_test.rs` covers the registry priority logic and the LRU/TTL cache
(`preview_test.rs:61-243`) but never exercises a concrete provider. None of `ArchivePreview`,
`CodePreview`, `ImagePreview`, `MediaPreview`, or `TextPreview` (`services/preview/providers/`) is
referenced by name in any test. The text, code, image, media, and archive preview paths, about
500 LoC of format-specific extraction, are only reached, if at all, through the previewer actor's
9 tests. Format parsing is exactly the kind of branchy code that wants per-provider table tests.
Severity Medium: a regression in one provider would not be caught until an actor-level test
happened to preview that format.

### Metadata extractors tested as a black box (Medium)

`metadata_test.rs` (51 tests) drives extraction through the public extract path, but does not name
any individual extractor (`AudioExtractor`, `VideoExtractor`, `DocumentExtractor`,
`ArchiveExtractor`, and the rest under `services/metadata/extractors/`). Coverage of the per-format
branches is therefore implicit and depends on the fixtures happening to hit each extractor. The
extractor set is the second-largest untested-by-name surface after preview providers. Severity
Medium.

### Previewer actor coverage is thin relative to its size (Medium)

The previewer is 664 LoC and owns two of the async defects CORE-008 flagged (unarmed metadata
dispatch at `previewer.rs:349`/`:399`, and the `remove`-clobber at `previewer.rs:204`/`:292`/
`:507`/`:584`), yet `previewer_test.rs` has only 9 tests, the lowest test-to-LoC density of any
actor. There is no test for rapid preview re-issue then cancel, which is the exact case the
clobber breaks. This overlaps REL-002's cancellation criterion; see the REL-002 section.

### Actor system shutdown and task-join are untested (Medium)

`actor_test.rs` (6 tests) covers the `Actor`/`ActorSystem` spawn and message path but not the
shutdown semantics CORE-008 called out: `shutdown()` aborts tracked loops but does not join the
detached per-command tasks (`actors/mod.rs:73`). No test asserts whether filesystem activity has
actually stopped after `FilerCore::shutdown` returns. This is a REL-002 cancellation/shutdown
concern; flagged here as a gap, owned there.

## Fixture and test-style inconsistencies

### `make_file` / `make_dir` are reimplemented with conflicting signatures

The most common fixture in the suite, a `FileNode` builder, exists in at least four mutually
incompatible shapes, each redefined locally per file:

- `make_file(name, size)` — `tests/model/directory_test.rs:13`
- `make_file(name, parent, size)` — `tests/model/session_manager_test.rs:135`, `tests/model/model_test.rs:78`
- `make_file(name, path, size, hidden)` — `tests/model/location_test.rs:19`, `tests/modules/search_test.rs:19`
- `make_file(name, size, hidden)` — `tests/model/query_test.rs:16`

Plus a long tail of near-duplicate variants: `make_file_at` (`directory_test.rs:65`),
`make_file_with_time` (`model_test.rs:131`), `make_file_with_ext` (`query_test.rs:36`),
`make_hidden` (`directory_test.rs:59`), `make_hidden_file` (`model_test.rs:98`), and
`make_hidden_dir` (`model_test.rs:125`). `make_dir` is similarly redefined in at least five files
with two different arities. A reader moving between test files cannot assume `make_file` means
what it meant in the previous file. There is no shared fixtures module: a search for
`mod common`, `mod helpers`, `mod fixtures`, or `mod test_util` returns nothing. The AGENTS.md
rule "check if there are shared logic can be extracted to a separate module" applies directly to
the test tree, and the test tree is the largest single violator. A `tests/support/` module
exporting one `FileNodeBuilder` would replace roughly a dozen ad-hoc constructors.

### Mock providers are duplicated across five files

`struct MockProvider` is defined independently five times: `scanner_test.rs:83`,
`search_test.rs:42`, `tests/navigation_flow_test.rs:31`, `tests/scanner_integration_test.rs:42`,
and `tests/search_integration_test.rs:28`. `struct MockFs` is defined twice (`vfs_test.rs:410`,
`tests/stress_test.rs:41`), and the operator and previewer suites add their own
`MockOpsProvider` (`operator_test.rs:46`) and `MockPreviewProvider` (`previewer_test.rs:58`).
These mocks implement the same `FsProvider` surface with slightly different behaviors, so a change
to the provider trait forces edits in seven places and the mocks can silently drift apart. A
single shared `MockProvider` with configurable behavior belongs in a test-support module.

### `build_core` harness is reimplemented, sometimes with the same name and different bodies

The full-stack harness that boots a `FilerCore` over a mock is duplicated: `build_core` in
`scanner_test.rs:147` takes a `MockProvider`, `build_core` in `search_test.rs:260` takes a
`MockFs`, and `build_core_with_search` in `search_test.rs:136` is a third variant. Same name,
different parameter types, different files. This is the harness-level echo of the mock and fixture
duplication.

### Timing-based synchronization makes async tests fragile

Async actor tests synchronize with the runtime using `sleep` plus `timeout` races rather than
deterministic signals. The density is concentrated in the actors that are hardest to test:
`watcher_test.rs` has 39 sleep/duration sites, `navigator_test.rs` 30, `command_router_test.rs`
16, and `scanner_test.rs` 14. A representative pattern in `watcher_test.rs`: fire an event, then
`sleep(Duration::from_millis(20)).await` (`watcher_test.rs:124`) or
`sleep(Duration::from_millis(100)).await` (`:223`), then race a `timeout(Duration::from_millis(100), ...recv)`
(`:144`, `:184`) to read the result. These pass today but are inherently load-sensitive: a busy CI
runner that delays the spawned task past the 100 ms window flips the test to a spurious failure,
and the fixed sleeps inflate wall-clock time for no functional reason. The watcher and navigator
suites are the ones to convert to event-driven synchronization (await the actual event/channel
with a generous outer deadline, not a fixed inner sleep). Severity Medium for reliability of the
suite itself.

### Doctests are written as non-running examples

Of the module-doc code blocks in `src/`, 11 are fenced ```` ```ignore ```` and only the handful of
plain-fenced blocks actually compile and run as doctests (3 passed). AGENTS.md asks module docs to
"add runnable code examples showing usage"; an `ignore` block is not verified, so the documented
usage can rot against the API. This is primarily a documentation-rule concern and belongs to
CORE-012; noted here because it is what the "11 ignored" doctest count represents.

### Dead test stub still in the tree

`src/tests/modules/navigation_flow_test.rs` is a one-line file whose only content is a comment
saying the tests "moved to filer-core/tests/navigation_flow_test.rs". It is not declared in
`tests/modules/mod.rs`, so it compiles into nothing and is pure dead weight. Delete it. Severity
Low, trivial cleanup.

## Overlap with REL-002

REL-002 ("Close core reliability coverage gaps") already owns the reliability-stress side of the
gaps above, and this report does not duplicate its criteria. The explicit overlaps:

- **Cancellation tests** for long operations, search, preview, and provider calls are REL-002
  criterion four. The cancellation-core gap (`cancel.rs` untested) and the thin previewer
  cancellation coverage feed that criterion; this report adds the finding that the *unit-level*
  primitive, not just the actor paths, is untested.
- **Watcher burst freshness and ordering** is REL-002 criterion three. The watcher's
  timing-fragile tests (`watcher_test.rs`) are the existing coverage REL-002 should harden.
- **Tracing on every command path** (REL-002 criterion two) and **cache-bypass on manual
  refresh** (criterion one) are not re-audited here; they remain REL-002's.

Where this report and REL-002 diverge: REL-002 is about *adding* reliability/stress coverage;
CORE-011 is about the *shape and consistency* of the existing suite (duplication, fragility, dead
code, untested primitives). Fixing the fixture/mock duplication is a CORE-011 follow-up, not
REL-002 scope.

## Missing test scenarios

Candidates, not findings:

- Unit-test `CancelMap`: `arm` twice for one session, assert the first token reads cancelled;
  spawn-order interleave where a stale task calls `remove` and assert `remove_if_current` would
  have preserved the live token. Pins the CORE-008 defect deterministically.
- Per-provider preview tests for text, code, image, media, and archive, asserting category,
  priority, and extracted payload for a known fixture file.
- Per-extractor metadata tests naming each extractor, so per-format branches are covered explicitly
  rather than by fixture accident.
- Previewer rapid re-issue then cancel for one session, asserting the latest in-flight task is
  cancelled (couples to the `remove_if_current` fix; overlaps REL-002).
- Shutdown-while-busy assertion that no filesystem activity continues after `FilerCore::shutdown`
  returns (overlaps REL-002 / CORE-008).

## Follow-up task candidates

Candidates for the CORE-013 remediation backlog, not new tasks created here.

- Add direct unit tests for `actors/cancel.rs` (`CancelMap` arm/is_latest/remove/remove_if_current
  interleavings). Severity: High. Highest-risk, least-covered code; couples to the CORE-008 fix.
- Extract a `tests/support/` module with one `FileNodeBuilder` and one configurable
  `MockProvider`/`MockFs`, replacing the four conflicting `make_file` signatures
  (`directory_test.rs:13`, `session_manager_test.rs:135`, `location_test.rs:19`,
  `query_test.rs:16`), the five `MockProvider` definitions, and the duplicated `build_core`
  harnesses. Severity: Medium. Largest maintainability win in the test tree.
- Convert the watcher and navigator suites from fixed `sleep` + `timeout` races
  (`watcher_test.rs`: 39 sites, `navigator_test.rs`: 30) to event-driven synchronization with a
  single outer deadline. Severity: Medium. Removes the suite's main flakiness vector.
- Add per-provider preview tests and per-extractor metadata tests so the ~500 LoC of preview
  providers and the metadata extractor set have direct, by-name coverage. Severity: Medium.
- Make module-doc examples runnable instead of `ignore` where the API allows, so documented usage
  is verified. Severity: Low. Overlaps CORE-012.
- Delete the dead stub `src/tests/modules/navigation_flow_test.rs`. Severity: Low.
