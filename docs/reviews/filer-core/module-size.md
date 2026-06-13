# Module Size and Decomposition Review (CORE-006)

Which filer-core files have outgrown the 700 LoC preferred ceiling or the 1000 LoC hard
limit, and where do their seams actually fall? This report inventories every file over 700
lines and proposes split boundaries grounded in cohesion, not line count alone. It is
review-only and changes no production code.

Evidence is cited as `path:line` against the crate at the time of review. Line counts are
total file lines.

## Inventory

Production modules over 700 LoC:

| File | Lines | Limit | Note |
| --- | --- | --- | --- |
| `modules/operations/operator.rs` | 1431 | over 1000 hard | Six fs operations, shared plumbing, and the dispatch loop in one file |
| `services/mime/table.rs` | 1041 | over 1000 hard | One sorted static data table; high lines, low complexity |
| `modules/scan/scanner.rs` | 994 | over 700 pref | A single 530-line `scan_directory` carries most of the weight |
| `modules/navigation/navigator.rs` | 707 | over 700 pref | A pure state machine and an async actor share one file |

Production modules named in scope but under the ceiling:

| File | Lines | Limit | Note |
| --- | --- | --- | --- |
| `modules/preview/previewer.rs` | 664 | under 700 | Single cohesive actor; Node/Location duplication, not size, is the smell |

Test files over 700 LoC (informational; the same size guidance applies to test modules):

| File | Lines |
| --- | --- |
| `tests/modules/scanner_test.rs` | 2980 |
| `tests/api/command_router_test.rs` | 2616 |
| `tests/modules/operator_test.rs` | 2020 |
| `tests/modules/search_test.rs` | 1925 |
| `tests/pipeline/pipeline_test.rs` | 1198 |
| `tests/modules/navigator_test.rs` | 1179 |
| `tests/infra/metadata_test.rs` | 777 |

The test files mirror the production hotspots one-to-one. They are out of scope for this
pass's split proposals, but they are flagged for CORE-011 (test-suite review): a test file
splits along the same behavioral seams as the module it covers, so the production splits
below give the test splits for free.

## Verdict

**Three production files breach a limit for real reasons and one breaches it on boilerplate.**
`operator.rs` and `scanner.rs` carry genuine logic that has natural cohesion seams, so they
should be split along those seams. `navigator.rs` has the cleanest seam of all: a pure state
machine sitting next to an async actor. `table.rs` breaches the hard limit but is one sorted
data table whose lines are repetition, not complexity, so its fix is mechanical and low risk.
`previewer.rs` is under the ceiling and should stay one module; its problem is duplication
between Node and Location code paths, which a refactor fixes without a split.

## operator.rs (1431) — split into command, handlers, and plumbing

The file holds four distinct concerns that today live together:

1. The command vocabulary: `OpsCommand` (`operator.rs:26`) and `CompletionShape`
   (`operator.rs:119`).
2. The `Operator` struct, constructors, arm/cancel bookkeeping, and location resolution
   (`operator.rs:125-210`, `operator.rs:993-1037`).
3. Six operation handlers that share one skeleton (resolve nodes, spawn a task, loop with a
   cancel check, emit progress, emit completion): `copy` (`operator.rs:212`), `moves`
   (`operator.rs:405`), `delete` (`operator.rs:558`), `rename` (`operator.rs:706`),
   `create_file` (`operator.rs:813`), `create_folder` (`operator.rs:903`).
4. Shared free-function plumbing: `copy_dir_recursive` (`operator.rs:1040`),
   `operation_complete_event` (`operator.rs:1105`), `emit_operation_progress`
   (`operator.rs:1134`), `is_cross_device` (`operator.rs:1153`), the cache-invalidation
   helpers (`operator.rs:1160-1174`), and `remove_operation_if_current` (`operator.rs:1176`).
5. The `Actor::run` dispatch loop, roughly 245 lines of match arms that route each Location
   variant through resolution into the corresponding Node handler (`operator.rs:1186-1424`).

Proposed module set under `modules/operations/operator/`:

- `command.rs` — `OpsCommand` and `CompletionShape`. The vocabulary changes for different
  reasons than the execution.
- `mod.rs` (or `operator.rs`) — the `Operator` struct, constructors, `arm_operation` /
  `cancel_operation` / `cancel_session`, `resolve_location_node(s)`, and the `Actor::run`
  dispatch loop. This is the orchestration shell.
- `transfer.rs` — `copy`, `moves`, and `copy_dir_recursive`. These three share the
  byte-moving concerns: recursive directory walk, cross-device fallback, per-item progress.
- `mutate.rs` — `delete`, `rename`, `create_file`, `create_folder`. Single-target structural
  mutations with collision and existence checks.
- `support.rs` — the shared plumbing helpers from concern 4.

Each resulting file lands well under 700. The seam is real: the six handlers already share an
identical skeleton, the helpers are already free functions taking explicit parameters (no
`&self`), so the move is mechanical. Splitting `transfer` from `mutate` groups by the shared
internal logic, not by alphabetical convenience.

Secondary observation for CORE-008: the dispatch loop's Location arms are near-identical
(resolve, then call the Node handler with `CompletionShape::Location`). A small generic
resolve-and-dispatch helper would shrink the loop, but that is a refactor, not a split.

Severity: **High**. Over the hard limit, and this file is the single point every filesystem
mutation flows through.

## mime/table.rs (1041) — separate data from logic, then compress the data

The file is one logical thing: the extension to MIME map. Of its 1041 lines, the struct
`ExtEntry` (`table.rs:15-20`) and `lookup_extension` (`table.rs:25-30`) are about 30 lines of
logic; the remaining ~1010 lines are the `EXT_TABLE` static (`table.rs:40-1041`), where each
of the ~125 entries spends eight lines on struct-literal boilerplate.

Cohesion here is maximal, so this is not a "break it into cohesive pieces" case. Two
constraints shape the fix:

- `lookup_extension` relies on `binary_search_by_key` (`table.rs:27`), which requires the
  table stay a single lexicographically sorted slice (`table.rs:34`). Splitting the data by
  category (text, image, audio) into separate slices would break that invariant or force a
  runtime merge. Do not split the data by category.
- The line count is repetition, not complexity. Each entry is reviewed at a glance.

Two recommendations, in order of value:

1. Preferred: compress the entry syntax while keeping one sorted slice. A declarative macro
   such as `ext!("png", "image/png", Image, Definitive)` or a flat
   `&[(&str, &str, MimeCategory, DetectionConfidence)]` tuple table collapses each entry from
   eight lines to one, dropping the file to roughly 200 lines and lowering review cost. The
   sorted-slice invariant is preserved; only the spelling changes.
2. Cheaper, if the macro is judged not worth it: move the `EXT_TABLE` literal into a sibling
   `table_data.rs` and keep `ExtEntry` plus `lookup_extension` in `table.rs`. This satisfies
   the size rule by separating the data file from the logic file, without touching the data
   shape. It treats the symptom, not the cause.

Severity: **Medium**. It breaches the hard limit, but the content is mechanical and the fix
carries little risk.

## scanner.rs (994) — extract the scan execution core

The file's weight is concentrated in one function. `scan_directory` (`scanner.rs:235-765`)
runs about 530 lines and holds three large, near-duplicated emission paths: serve-from-cache
(`scanner.rs:320-440`), the paged provider load (`scanner.rs:459-578`), and the full provider
load (`scanner.rs:580-764`). The `emit_scan_progress(... ProgressSnapshot::new(...))` call
recurs about fifteen times with only the status, phase, and counts changing.

The rest of the file separates cleanly into two layers:

- The actor shell: `ScanCommand` (`scanner.rs:27`), the `Scanner` struct and constructors
  (`scanner.rs:69-116`), the `dispatch_*` methods that resolve a command to a path and spawn
  (`scanner.rs:124-233`), `cancel_scan` (`scanner.rs:899`), and `Actor::run`
  (`scanner.rs:905-994`).
- The execution core: `scan_directory`, `emit_page_result` (`scanner.rs:767`),
  `emit_scan_progress` (`scanner.rs:861`), `scan_target` (`scanner.rs:882`), and `is_latest`
  (`scanner.rs:889`). These are all associated functions that already take explicit borrowed
  parameters rather than `&self`, so they detach with no struct refactor.

Proposed split:

- `scanner.rs` — `ScanCommand`, the `Scanner` struct and constructors, the `dispatch_*`
  resolution/spawn methods, `cancel_scan`, and `Actor::run`.
- `scan_exec.rs` — `scan_directory`, `emit_page_result`, `emit_scan_progress`, `scan_target`,
  `is_latest`.

This alone moves roughly 530 lines out of `scanner.rs`. The higher-value follow-up, owned by
CORE-008, is to decompose `scan_directory` itself into `serve_from_cache`,
`load_paged`, and `load_full` helpers and to fold the repeated progress emission into a small
builder. The cohesion inside the function is the three load strategies; that is where it wants
to break.

Severity: **High**. Near the hard limit, and the single 530-line function is the real
maintainability cost, not the file total.

## navigator.rs (707) — split the state machine from the actor

This file has the cleanest seam in the crate. Two concerns sit side by side with no shared
mutable coupling:

- A pure per-session state machine: `NavState` (`navigator.rs:78-109`) and `NavigatorState`
  with its history `VecDeque`, `navigate` / `back` / `forward` / `can_back` / `can_forward` /
  `snapshot` (`navigator.rs:112-272`). This code touches no `Sender`, no `ScanCommand`, and no
  `Event`. It is synchronous and unit-testable in isolation.
- The async actor: the `Navigator` struct, `handle_command`, the `trigger_*` scan helpers, and
  `Actor::run` (`navigator.rs:275-707`). This depends on the state machine, not the reverse.

Proposed split:

- `nav_state.rs` — `NavState` and `NavigatorState`.
- `navigator.rs` — the `Navigator` actor.

The dependency points one way (actor uses state), the move is mechanical, and it drops
`navigator.rs` to roughly 430 lines while giving the history logic its own focused test
surface.

Severity: **Medium**. Just over the preferred ceiling, with an obvious and low-risk seam.

## previewer.rs (664) — keep as one module, remove the duplication

`previewer.rs` is under the 700 ceiling and is a single cohesive actor handling three
operation types: preview generation, basic metadata, and extended metadata. Splitting it by
Node versus Location would scatter one operation across files and fight the cohesion. The
documented recommendation is to keep it as one module.

The file's actual smell is duplication, not size. Each operation has a Node variant and a
Location variant that differ only in the resolve step and the event shape: `dispatch_preview`
(`previewer.rs:111`) versus `dispatch_preview_location` (`previewer.rs:208`),
`dispatch_metadata` (`previewer.rs:330`) versus `dispatch_metadata_location`
(`previewer.rs:379`), and `dispatch_extended_metadata` (`previewer.rs:429`) versus
`dispatch_extended_metadata_location` (`previewer.rs:511`). The MIME detect-with-fallback
block is copied verbatim between the two extended-metadata methods (`previewer.rs:458-471` and
`previewer.rs:539-551`).

Recommendation, owned in spirit by CORE-007: extract a shared `detect_mime(provider, path)`
helper and collapse each Node/Location pair so the differing resolve and event-emit steps are
the only variation. This lowers both the line count and the review cost without a module
split. If the file later grows past the ceiling after providers expand, revisit a split by
operation type (preview, metadata, extended-metadata), which is the only seam that respects
cohesion.

Severity: **Low**. Under the ceiling; the work is a dedup refactor, not a decomposition.

## Follow-up task candidates

These are candidates for the CORE-013 remediation backlog, not new tasks created here.

- Split `operator.rs` into `command` / `transfer` / `mutate` / `support` / dispatch modules.
  Severity: High.
- Decompose `scan_directory` into cache/paged/full helpers and extract a `scan_exec` module.
  Severity: High. Joint input with CORE-008.
- Split `navigator.rs` into `nav_state` and the actor. Severity: Medium.
- Compress `EXT_TABLE` entry syntax (macro or tuple form), or at minimum move the data into
  `table_data.rs`. Severity: Medium.
- Deduplicate the Node/Location operation pairs in `previewer.rs` and extract a shared
  `detect_mime` helper. Severity: Low. Feeds CORE-007.
- Flag the oversized test files for split alongside their production modules. Severity: Low.
  Feeds CORE-011.
