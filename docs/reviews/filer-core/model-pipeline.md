# Data Model and Pipeline Contracts Review (CORE-010)

Do the core data types (`Location`, `Node`, `Query`), the `Pipeline` transform stages, the
`GroupedNodes` contract, and the keyset cursor form a foundation that produces correct, stable,
reproducible directory views as the project scales to large directories and live mutation? This
report evaluates the model types every frontend consumes, the transform pipeline that
PIPELINE-TRANSFORMS requires all filtering, sorting, and grouping to flow through
(`docs/architecture/invariants.md`), the `GroupedNodes` output shape, and cursor stability under
directory mutation. It is review-only and changes no production code.

Evidence is cited as `path:line` against the crate at review time. Test modules under `src/tests/`
are out of scope except where they pin a contract.

## The data model in one paragraph

The model splits into identity, rows, and query. Identity is `Location` plus `LocationDescriptor`
(`model/location.rs:29-52`): a scheme, a `ProviderRef`, a root path, an ordered `segments` chain,
and a hashed `LocationId`. `LocationRef` carries id, descriptor, or both for transport. Rows come in
two parallel shapes: `FileNode`, the path-era row that providers and the pipeline operate on
(`model/node.rs:30-40`), and `NodeEntry`, the Location-native public row
(`model/node.rs:48-58`), with `NodeEntry::from_file_node` bridging the two. Query is `SearchQuery`,
a parsed bundle of `text`, `Vec<QueryFilter>`, and `SearchOptions` (`model/query.rs:26-53`). The
pipeline transforms `Vec<FileNode>` through `Stage` objects into `PipelineData` (flat or
`GroupedNodes`), built from a serializable `PipelineConfig` (`pipeline/mod.rs:112-131`,
`pipeline/config.rs:46-57`). The model is coherent and honestly typed. Its weaknesses are not in the
type shapes; they are in two places where the same concept is implemented twice and the two copies
disagree, and in config fields that the pipeline silently never honors.

## Listing order is defined twice and the two definitions disagree

This is the highest-severity finding and the one that directly threatens cursor stability.

There are two independent authorities for "what order are the rows in." The full-snapshot path uses
the `SortBy` stage (`pipeline/sort.rs:48-73`), reached through `Pipeline::execute_grouped`
(`scanner.rs:359`, `:667`, `:800`). The paged path uses `compare_nodes` (`pipeline/order.rs:25-59`)
to keyset-insert rows in `PageSelection::extend` (`paging.rs:297`, `:304`). The scanner chooses
between them per request: a page request routes to `paging.load_cached`/`load_provider`
(`scanner.rs:324`), everything else to `execute_grouped().limited()` (`scanner.rs:357-360`). So the
same directory, with the same `PipelineConfig`, is ordered by `SortBy` when loaded as a snapshot and
by `compare_nodes` when paged. They must agree. They do not.

- **Tie-breaking diverges.** `compare_nodes` ends with `.then_with(name).then_with(path)`
  (`order.rs:57-58`), a total, deterministic order. `SortBy::sort_nodes` has no tie-breaker; on equal
  sort keys it returns `Ordering::Equal` and leans on `sort_by` stability (`sort.rs:49-71`), so the
  residual order is the provider's `read_dir` order. For a `Size` or `Modified` sort with repeated
  keys, the snapshot order is nondeterministic input order while the paged order is deterministic by
  name then path. Same data, two orders.
- **Extension ordering is inverted.** `compare_nodes` does `left.extension().cmp(&right.extension())`
  (`order.rs:49`); `Option` ordering puts `None` before `Some`, so extensionless rows sort first.
  `SortBy` instead maps `(Some, None) => Less` and `(None, Some) => Greater` (`sort.rs:64-65`), so
  extensionless rows sort last. The two paths order an extension sort in opposite directions. The
  `SortBy` arms also pass through `handle_order` while its `(None, None) => Equal` arm does not
  (`sort.rs:62-66`), so descending order flips some tie classes and not others.

A user who scrolls a large directory (paged) sees one order; the same directory loaded under the
snapshot limit shows another. Worse for criterion two: the cursor's stability guarantee lives
entirely in `compare_nodes`'s path tie-breaker, and the stage that orders a full load does not share
it. Any code path that mixes a snapshot first page with paged continuations inherits the mismatch.
PIPELINE-TRANSFORMS says ordering flows through the pipeline; today it flows through two pipelines
that disagree. The fix is to delete `SortBy::sort_nodes`'s ordering logic and have both paths call
`compare_nodes` as the single comparator. Severity high.

## Group display order ignores the `sort_order` the model already defines

`GroupBy` builds groups into a map, then orders them by `a.label.cmp(&b.label)`, a lexicographic
sort of the human display string (`pipeline/group.rs:68`). For extension and first-letter grouping
that is acceptable. For date and size grouping it is wrong, and the correct order already exists and
is thrown away.

`TimeGroup::sort_order` returns 0 for `LastHour` up to 8 for `Unknown` (`utils/time.rs:164-176`), but
grouping keys on `display_name` (`order.rs:68`, `time_group_name`), so the labels sort as "Last 10
years", "Last hour", "Older", "This month", "This week", "This year", "Today", "Unknown",
"Yesterday". That is alphabetical, not chronological. `SizeGroup::sort_order` runs 0 (`Empty`) to 6
(`Massive`) (`utils/size.rs:139-148`), but labels sort as "Empty", "Huge (1 GB - 10 GB)", "Large...",
"Massive...", "Medium...", "Small...", "Tiny...". A "Massive" group renders above "Medium". The
dedicated `sort_order` methods exist precisely for this and are never called. `compare_nodes` shares
the defect: it uses `group_label(...).cmp(group_label(...))` as its primary key (`order.rs:26`), so
the paged path orders date and size groups by the same wrong alphabetical key. The two paths agree
here only because they are wrong the same way. Fix: key group ordering on `sort_order`, not the
display string. Severity medium, a visible correctness bug for two of five grouping modes.

## `FilterConfig` size and name filters are accepted, influence paging, and never filter

`FilterConfig` exposes `min_size`, `max_size`, and `name_pattern` (`config.rs:167-177`). No stage
applies them. `Pipeline::from_config` builds only `FilterHidden` and `FilterByExtension` and leaves
the rest as `// - min_size / max_size` / `// - name_pattern` TODO comments (`mod.rs:160-168`). The
only `Stage` implementations in the crate are `FilterHidden`, `FilterByExtension`, `SortBy`, and
`GroupBy` (`filter.rs:22`, `:72`, `sort.rs:76`, `group.rs:26`). So these three fields are dead as
filters.

They are not dead as signals, which makes the gap worse than a missing feature. `paging_mode`
returns `SnapshotOnly` when any of them is set (`config.rs:117-127`), and `effective_listing` forces
a full-metadata listing when `min_size`/`max_size` is present (`order.rs:91-94`). So a request with
`min_size: Some(...)` pays for full metadata extraction and forfeits provider paging, then returns
rows that were never filtered by size. The config promises a filter, charges the cost of honoring
it, and delivers unfiltered results. `SearchQuery` implements size and name filtering correctly and
separately (`query.rs:390-406`), which both proves the logic exists and shows it lives in the wrong
place to be reused. Either implement the three stages or remove the fields and their paging
influence; do not keep config that lies. Severity medium, a silent correctness gap.

## Two definitions of "hidden" disagree, and the pipeline one is wrong on Windows

A node's hidden state is computed once at construction into `NodeMeta::hidden`, using the
platform-correct rule: dotfile on Unix, `FILE_ATTRIBUTE_HIDDEN` on Windows (`node.rs:137-144`,
`:221-228`). `QueryFilter::IsHidden` reads that field (`query.rs:402`). But the pipeline's
`FilterHidden` ignores `meta.hidden` and recomputes from the path with `is_hidden(f.path)`
(`filter.rs:17`), and `is_hidden` only checks for a leading dot, walking parents
(`utils/path.rs:17-28`). On Windows, `FILE_ATTRIBUTE_HIDDEN` files are not dotfiles, so `FilterHidden`
does not hide them, while `QueryFilter::IsHidden` does. The listing filter and the search filter
disagree about what "hidden" means, and the listing filter is wrong on the project's stated primary
platform (Windows 11). The path also walks every ancestor for each node (`path.rs:23-25`) even though
all rows in one listing share a parent, recomputing a constant. Fix: filter on the already-computed
`meta.hidden` so one platform-correct definition serves both filter and query. Severity medium.

## Cursor stability under directory mutation

The cursor is not self-describing. It is an opaque server-side handle, `paging:v1:N` from a global
counter (`paging.rs:28`, `:315-321`), that keys a `PagingSession` in a process-wide map
(`paging.rs:42-44`). The session stores the full last-emitted `FileNode`, the frozen `total_count`,
the `start_index`, and the request/pipeline it was issued under (`paging.rs:31-39`). Continuation
re-selects rows strictly greater than the stored `last` per `compare_nodes` (`paging.rs:294-300`).
This design is sound in its core choice: a keyset cursor over a stable total order survives
insertion and deletion far better than an offset. The failure modes are specific and worth
documenting for criterion two.

- **The cursor is single-use.** `continuation` removes the session from the map as soon as it reads
  it (`paging.rs:200-204`). A client that retries a page after a dropped response, or replays the
  same cursor, gets "Expired directory paging cursor" (`paging.rs:190`). Pagination is therefore
  non-idempotent and fragile to transport loss. Severity medium.
- **The keyset row is a stale snapshot.** The stored `last` carries the sort-key values it had when
  the page was issued. If the sort field is `Size` or `Modified` and that file is rewritten between
  pages, `compare_nodes` compares fresh rows against the old key. A file whose key moved forward can
  reappear on a later page (duplicate); a file whose key moved backward across the boundary can be
  skipped (omission). With a `Name` sort the key is the path, which mutation does not change except
  by rename, so the default view is robust; the exposure is exactly the metadata sorts that also
  force `SnapshotOnly` elsewhere. This is the central cursor-stability-under-mutation failure mode
  and it is currently undocumented. Severity medium, realized only under concurrent metadata change
  on a metadata sort.
- **`total_count` is frozen for the whole sequence.** `finish_page` reuses the first page's
  `total_count` on every continuation (`paging.rs:222-226`, `:247`). Under mutation the reported
  total drifts from the number of rows actually delivered. This is a deliberate stability choice and
  the right default, but the contract that "total is a first-page estimate, not a running count"
  should be stated. Severity low.
- **Tie-break correctness depends entirely on `compare_nodes`.** Because keyset selection uses
  `compare_nodes` and that comparator has a total path tie-breaker, the cursor boundary is
  unambiguous even when visible fields tie. This is the part that works, and it is the reason the
  divergent `SortBy` order (first finding) is dangerous: any reordering not done through
  `compare_nodes` breaks the boundary guarantee.

## The paging session map has no eviction and grows with abandoned pagination

Sessions are inserted on every page that has more (`paging.rs:232-249`) and removed only on
continuation (`paging.rs:200-204`) or by `clear_session`, which fires on a fresh cursorless load
(`paging.rs:79`, `:153`) or session teardown (`paging.rs:56-61`). A client that loads page one and
never requests page two leaves the session resident until that owner starts another listing or the
session ends. There is no TTL and no cap on map size. Each entry holds a full `FileNode` clone plus a
cloned `PipelineConfig`. Across many sessions and abandoned scrolls the map grows unbounded. This is
a slow leak, not a crash, but the "performance first, reliability second" priority argues for a
bounded LRU or a TTL sweep. Severity medium for a long-lived process.

## Stages clone every node they touch; selection runs the whole pipeline per row

The `Stage` trait takes `PipelineData` by value (`mod.rs:120`), so a stage owns its input and can
move it. The grouped branches instead clone: `FilterHidden` and `FilterByExtension` do
`group.nodes = self.filter_nodes(group.nodes.clone())` (`filter.rs:29`, `:79`), and `SortBy` does
`group.nodes = self.sort_nodes(group.nodes.clone())` (`sort.rs:83`). Each clones a `Vec<FileNode>`,
and `FileNode` owns a `String` name and a `PathBuf`, so this is a deep per-row allocation that
`std::mem::take(&mut group.nodes)` would remove. This conflicts with the no-unnecessary-clone rule
(AGENTS Rust rules). Separately, `PageSelection::extend` calls `self.pipeline.execute_flat(vec![entry])`
once per directory row (`paging.rs:289`), allocating a one-element `Vec` and walking every stage for a
single node; the `SortBy` and `GroupBy` stages do nothing useful on one element since the real order
comes from the `compare_nodes` binary-insert that follows. For the large-directory proof target this
is per-row allocation plus dead stage work on the hot path. Severity medium for the performance
target, low for correctness.

## `NodeId::from_path` panics on a non-UTF-8 path

`NodeId::from_path` hashes `path.to_str().unwrap()` (`node.rs:414`). A non-UTF-8 path, which is legal
on both Linux and Windows, panics. `NodeId` is on the hot construction path: `from_path`,
`from_metadata`, and `from_dir_entry` all call it when no registry is supplied (`node.rs:109`,
`:193`, `:278`). This violates the no-`unwrap`-in-production rule (AGENTS Rust rules) and is a real
panic surface for an explorer that must list arbitrary filesystems. Hash the raw bytes via
`path.as_os_str().as_encoded_bytes()` instead, which never fails. Severity medium, a latent panic on
valid input.

## Lower-severity model and query notes

These are contract-shaping observations, not present blockers.

- **`FileNode` and `NodeEntry` are near-duplicate rows** (`node.rs:30-58`) with three constructor
  bodies (`from_path`, `from_metadata`, `from_dir_entry`) that repeat name extraction, id
  assignment, kind classification, and platform hidden/permission logic almost verbatim
  (`node.rs:76-315`). The doc comments already mark `FileNode` as legacy and steer new APIs to
  `NodeEntry`. Extracting the shared metadata-to-fields logic and planning the `FileNode` retirement
  would cut the duplication the AGENTS maintainability rule warns about. The split also means the
  pipeline operates on `FileNode` and converts to `NodeEntry` only at the edge (`mod.rs:50-70`),
  which is a reasonable seam to keep.
- **`GroupBy::Type` silently means "by extension."** Both `from_config` (`mod.rs:188`) and
  `group_label` (`order.rs:64`) map `Type` to extension with a "for now" intent. Grouping by type
  produces extension groups under a type label. Either implement MIME-category grouping (the
  `services/mime` table exists) or remove the variant; do not ship a mislabeled grouping. Severity
  low.
- **`QueryFilter::NameMatches` recompiles its regex per node.** `matches` calls
  `regex::Regex::new(pattern)` for every node (`query.rs:404`), discarding the compile that already
  happened and was validated at parse time (`query.rs:107`). For a search over a large tree this
  recompiles the same pattern thousands of times. Carry the compiled `Regex` in the parsed filter.
  Severity low to medium for the search performance target.
- **Two filtering systems do not share logic.** `QueryFilter` (search) and the `FilterConfig`
  stages (listing) both express extension, size, and name filtering, in different code with
  different semantics (`query.rs:382-408` vs `filter.rs`). The size and name gaps above are a direct
  consequence. A shared predicate layer that both the search matcher and the pipeline stages consume
  would satisfy PIPELINE-TRANSFORMS and the maintainability rule at once. Severity low as design
  debt.
- **`GroupedNodes::get` matches labels case-insensitively** (`utils/grouped_node.rs:4-13`). Two
  distinct groups whose labels differ only by case would collide on lookup. Harmless for current
  label sets, worth noting if label sources widen.

## Summary table

| Axis | State | Severity |
| --- | --- | --- |
| Listing order authority | Defined twice (`SortBy` vs `compare_nodes`); disagree on ties and extension | High |
| Group display order | Keys on display string, ignores `sort_order`; date/size groups misordered | Medium |
| `FilterConfig` size/name filters | Accepted, force `SnapshotOnly` and metadata listing, never applied | Medium |
| "Hidden" definition | Pipeline recomputes from path, wrong on Windows; diverges from `meta.hidden` | Medium |
| Cursor under mutation | Keyset sound; single-use, stale-key duplicate/skip on metadata sorts, frozen total | Medium |
| Paging session map | No TTL/cap; grows with abandoned pagination | Medium |
| Pipeline clones / per-row execute | Grouped stages clone; `extend` runs full pipeline per row | Medium (perf) |
| `NodeId::from_path` | `unwrap` on non-UTF-8 path panics on valid input | Medium |
| `FileNode`/`NodeEntry` + constructors | Near-duplicate rows and constructor bodies | Low (maintainability) |
| `GroupBy::Type` | Silently maps to extension | Low |
| `NameMatches` regex | Recompiled per node | Low–Medium (search perf) |

## Follow-up task candidates

Candidates for the CORE-013 remediation backlog, not new tasks created here.

- Unify listing order on a single comparator: make the `SortBy` stage and the paged path both call
  `compare_nodes`, fixing the extension-direction and tie-break divergence. This is the prerequisite
  for cursor stability and directly serves PIPELINE-TRANSFORMS. Severity: High.
- Order date and size groups by `TimeGroup::sort_order`/`SizeGroup::sort_order` instead of the
  display string, in both `group.rs` and the `group_label` key in `compare_nodes`. Severity: Medium.
- Decide the fate of `FilterConfig::min_size`/`max_size`/`name_pattern`: implement the stages or
  remove the fields and their `paging_mode`/`effective_listing` influence. Pair with a shared
  predicate layer reused by `SearchQuery`. Severity: Medium.
- Filter hidden files on `NodeMeta::hidden` so listing and search share one platform-correct
  definition and the Windows hidden-attribute case is honored. Severity: Medium.
- Document the cursor contract and harden it: state that the keyset row is a point-in-time snapshot
  (duplicate/skip possible on metadata sorts under mutation) and that `total_count` is a first-page
  estimate; consider making cursors replay-tolerant rather than single-use. Joint with CORE-008.
  Severity: Medium.
- Bound the paging session map with a TTL or LRU so abandoned pagination cannot grow memory without
  limit. Severity: Medium.
- Remove the grouped-branch `.clone()` in the filter and sort stages via `mem::take`, and avoid the
  per-row `execute_flat(vec![entry])` on the paging hot path. Joint with the large-directory work in
  CORE-008. Severity: Medium for the performance target.
- Replace `NodeId::from_path`'s `unwrap` with byte-based hashing so non-UTF-8 paths cannot panic.
  Severity: Medium.
- Plan `FileNode` retirement toward `NodeEntry` and extract the shared node-construction logic.
  Severity: Low.
- Carry a compiled `Regex` in `QueryFilter::NameMatches` instead of recompiling per node. Severity:
  Low–Medium.
- Implement real MIME-category grouping for `GroupBy::Type` or remove the variant. Severity: Low.
