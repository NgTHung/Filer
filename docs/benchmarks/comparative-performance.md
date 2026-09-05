# Comparative Performance Benchmark Design

## Purpose

This suite tells you whether Filer-core is becoming faster, whether it is
competitive with relevant libraries and file-manager frameworks, and where
time is spent across a complete user action.

Keep three result sets separate:

1. Engine results compare directory and search implementations in process.
2. Core results measure Filer through public commands and events.
3. Application results measure input to a correct visible frame.

Do not combine these layers into one score. Process startup, terminal drawing,
provider work, pipeline work, and output formatting answer different questions.

## Scope

The first implementation stays under Filer-core. It may include a reference
client that consumes public core events and commits a deterministic virtual
view. It must not require changes to filer-app.

External application adapters run released binaries in isolated environments.
They are benchmark tools, not Filer-core dependencies. Full filer-app coverage
can use the same protocol when the full application work resumes. The separate
app:UI-011 validation track supplies real-window feedback during 0.3.1 and does
not become a dependency of this benchmark package.

The full program covers the scenarios below. The 0.3.1 slice implements only
flat-10k/flat-100k, fast and metadata browsing, continuation, and one journey
through paging, name sorting, filtering, and refresh. CORE-029 owns the full
program; its completion is outside the 0.3.1 exit gate. CORE-042 owns the
remaining fixtures, journeys, and internal trace attribution.

The full suite covers:

- flat directory browsing and continuation
- metadata enrichment
- sorting, filtering, and grouping
- recursive search, first match, completion, and cancellation
- navigation sequences, refresh, and cache reuse
- filesystem mutation convergence
- semantic decorations through the completed MODULES-002 contract
- input responsiveness while long work is active

## Benchmark Layers

| Layer | Filer path | Comparison set | Boundary |
|---|---|---|---|
| Engine | Local provider and pipeline functions | `std::fs`, Tokio, GIO, KIO | Adapter call to canonical rows |
| Recursive search | Search provider and matcher | `walkdir`, `jwalk`, `ignore::WalkParallel` | Search request to canonical matches |
| Core | `Command` to terminal `Event` | Filer revisions | Public command and event contract |
| Reference application | Semantic input to virtual-view commit | Filer revisions | Input injection to correct committed view |
| External application | Process or steady-state input to visible frame | Yazi and Broot | Black-box input to correct terminal frame |

GIO and KIO are framework comparisons, not whole-application comparisons.
Yazi and Broot are whole applications, so their results never share an
in-process leaderboard with Filer-core.

## Initial Competitors

### Flat Directory Engines

- `std::fs::read_dir` provides the synchronous Rust lower bound.
- `tokio::fs::read_dir` shows the async runtime cost Filer already builds on.
- GIO `GFileEnumerator` provides synchronous and batched asynchronous listing
  with selectable metadata attributes.
- KDE `KCoreDirLister` provides incremental items, completion, cancellation,
  filtering, and update signals without a graphical view.

### Recursive Search Engines

- `walkdir` provides a simple sequential traversal reference.
- `jwalk` provides parallel traversal for multi-directory trees.
- `ignore::WalkParallel` provides parallel traversal with hidden, glob, and
  ignore-file behavior.

Do not use recursive walkers as flat-directory competitors. Parallel walkers
solve a different problem when only one directory is read.

### Whole Applications

- Yazi is the primary speed-focused Rust file-manager comparison.
- Broot is the primary tree-navigation and interactive-search comparison.

Pin every application by release and binary digest. Record its configuration
with each run. If an application cannot express a scenario with matching
semantics, report `not_supported`. Do not give it a zero or estimate the result.

## Versioned Protocol

Every adapter reads one scenario request and writes newline-delimited JSON
events. A protocol version prevents an old adapter from silently producing an
incompatible result.

A run request contains:

- protocol version and scenario id
- fixture manifest id and digest
- implementation id, version, and build profile
- cache mode
- page and viewport size
- requested fields, sort, filter, group, and search configuration
- sample, process, and randomized-order identifiers

Each event contains:

- run and sample id
- monotonic timestamp in nanoseconds
- phase name
- rows examined, accepted, emitted, and visible when known
- output digest when the phase commits semantic state
- structured error or cancellation status

Use a canonical row shape for comparison. Include only fields required by the
scenario. A fast-listing adapter must not lose because another adapter gathered
metadata that the scenario did not request.

## Correctness Before Timing

Every timed sample must prove semantic equivalence.

The fixture manifest records expected digests for:

- complete row membership, independent of unspecified provider enumeration order
- sorted rows
- grouped labels and group order
- filtered rows
- search matches
- visible viewport state after each scripted action

An adapter result is invalid when its digest, row count, completion state, or
error behavior differs. Invalid results never appear in a performance ranking.

Use fixture-relative identities for membership digests. Check order only when
the scenario requests it. For provider-order pages, validate visible rows against
the adapter's observed sequence and validate uniqueness and membership across
the completed chain. Different filesystem enumeration orders remain valid.

For mutable scenarios, record the expected final generation and digest. Measure
convergence only after correctness identifies the authoritative generation.

## Fixture Corpus

Generate fixtures from versioned manifests so every adapter sees the same
names, types, metadata, permissions, and mutations.

| Fixture | Purpose |
|---|---|
| `flat-10k` | System32-scale first paint and paging |
| `flat-100k` | Scaling and accidental full-walk detection |
| `tree-100k` | Recursive traversal and search |
| `sparse-match-100k` | Filters and searches whose matches appear late |
| `git-10k` | Clean, modified, added, deleted, ignored, untracked, and conflicted rows |
| `hostile-10k` | Unicode, non-UTF-8 Unix names, symlinks, long names, and permission errors |
| `mutation-10k` | Create, delete, rename, and metadata changes during an active view |

Use a realistic mix of files and directories. Vary extensions, sparse sizes,
timestamps, hidden state, and permissions. Fixture creation is never part of a
timed sample.

Record the filesystem type and mount options. Keep tmpfs, Btrfs, ext4, APFS,
and NTFS results separate.

## Scenario Contracts

Each scenario defines its start barrier, timed milestones, terminal condition,
correctness digest, and supported benchmark layers.

### Browse

| Scenario | Action | Milestones |
|---|---|---|
| `browse.fast.first` | Open `flat-10k` without metadata | first row, viewport, page, complete |
| `browse.fast.scale` | Open `flat-100k` | viewport, page, complete |
| `browse.metadata.first` | Open with size and timestamps | viewport, page, complete |
| `browse.next` | Request pages 2, 10, and final | page commit for each request |
| `browse.refresh` | Refresh a warm location | refreshed viewport and complete state |

### Presentation Pipeline

| Scenario | Action | Terminal condition |
|---|---|---|
| `view.sort.name` | Sort by name | correct visible order committed |
| `view.sort.size` | Sort by size | correct metadata-backed order committed |
| `view.filter.common` | Filter with 50 percent selectivity | full viewport committed |
| `view.filter.sparse` | Filter with 0.01 percent selectivity | terminal page or completion committed |
| `view.group.extension` | Group by extension | group labels, order, and viewport committed |
| `view.group.size` | Group by size | metadata-backed groups committed |

### Search

| Scenario | Action | Milestones |
|---|---|---|
| `search.early` | Find a root-near exact match | first match and completion |
| `search.late` | Find a deep late match | first match and completion |
| `search.common` | Match about 10 percent | first batch, viewport, completion |
| `search.none` | Match nothing | completion |
| `search.cancel` | Cancel after a fixed barrier | cancel accepted, work stopped, quiescent |
| `search.concurrent` | Run four session-isolated searches | per-session first match and completion |

### Whole-Pipeline Journeys

Journeys keep one application process alive and exercise the full state
machine. They expose cache, queue, rendering, and cancellation behavior that an
isolated operation misses.

`journey.browse-organize-search` performs:

1. Navigate to `flat-10k`.
2. Commit the first viewport.
3. Sort by size.
4. Group by extension.
5. Apply a sparse filter.
6. Clear the filter and request page 10.
7. Navigate into `tree-100k`.
8. Start a common search and render its first viewport.
9. Replace it with a rare search.
10. Cancel, navigate back, and refresh.

`journey.mutation-recovery` opens `mutation-10k`, applies the scripted
filesystem changes, and waits for the correct final view without duplicate or
missing rows.

`journey.decorated-browse` opens `git-10k`, commits undecorated listing rows,
then commits semantic decorations. Listing delivery must not wait for Git work.

## Rigorous Application Measurement

Application benchmarking uses an instrumented view and a black-box view at the
same time.

### Black-Box View

The driver launches or connects to the application, waits at a ready barrier,
injects a semantic action, and observes the first correct visible frame.

For terminal applications:

- use a pseudoterminal with fixed dimensions and terminal capabilities
- isolate `HOME`, XDG configuration, cache, and state directories
- parse ANSI output into a virtual terminal instead of matching raw bytes
- hash the visible rows, selection, group labels, and status state
- retain the terminal transcript when a sample fails

For a future graphical Filer application, use the platform accessibility tree
for semantic view correctness and a frame-commit marker for presentation time.
Screen-image matching is a diagnostic artifact, not the primary correctness
oracle.

The black-box metrics are fair across applications:

- process start to first correct frame
- ready application input to first changed correct frame
- input to stable terminal state
- peak resident memory and CPU time

### Instrumented Filer View

Filer also emits benchmark trace events for attribution. These events carry ids
and timestamps but do not change command or result semantics.

Use these phases:

1. `input.injected`
2. `command.sent`
3. `router.accepted`
4. `provider.started`
5. `provider.first_batch`
6. `provider.completed`
7. `pipeline.started`
8. `pipeline.completed`
9. `event.enqueued`
10. `event.received`
11. `view.committed`
12. `frame.committed`
13. `work.quiescent`

Derive provider, pipeline, queue, view-update, and render durations from one
monotonic clock domain. Propagate the run, action, session, and request ids
through every phase. Missing or duplicate phases required by the selected
scenario invalidate the sample. CORE-042 owns this internal attribution;
the initial CORE-032 journey requires only its public input/event/view markers.

The reference application is a deterministic consumer of public Filer-core
events. It maintains viewport, selection, sorting, filtering, grouping, and
search state, then commits a virtual frame. This gives Filer-core a rigorous
input-to-view benchmark independently of the app:UI-011 desktop validation
track. Keep its virtual-view measurements separate from that track's actual
window/frame observations. A later benchmark adapter can measure the real app.

### Responsiveness Under Work

Throughput alone does not prove that browsing feels fast. While a browse,
search, metadata, or decoration action runs, inject a lightweight focus or
selection action at a fixed interval.

Record:

- input-to-frame median, p95, and p99
- maximum main-loop stall
- event queue high-water mark
- frames committed and superseded
- stale events rejected
- time from cancel acceptance to quiescence
- results or filesystem work observed after cancellation

This measurement catches a fast total completion that still freezes input.

## Metrics

### Latency

- time to first row
- time to first viewport, default 40 rows
- time to first page, default 256 rows
- time to first search match and first search viewport
- time to complete
- input to correct frame
- cancellation acceptance to quiescence
- mutation to correct converged frame

### Work and Resources

- rows examined, accepted, emitted, and visible
- work amplification, `rows examined / rows visible`
- CPU time
- peak resident memory
- allocation count and allocated bytes when supported
- directory-read and metadata syscall counts when supported
- event and frame counts

### Reliability

- output digest and row-count agreement
- duplicate or missing rows across pages
- stale events after superseding actions
- work observed after cancellation
- final view generation after mutation

## Sampling and Cache Policy

Report cold-start, warm-start, and warm steady-state results separately.

- Cold-start samples use independent processes and isolated application state.
- Cold-filesystem samples use a fresh fixture copy or an explicitly recorded
  platform cache reset. Never require privileged cache dropping in normal CI.
- Warm samples run a declared warmup before timing.
- Steady-state journeys reuse one process but reset semantic state at a barrier.

Use at least 20 independent process samples for startup results. Use at least
five processes with ten randomized actions each for steady-state journeys.
Randomize implementation order within each round.

Report median, p95, p99 when the sample count supports it, median absolute
deviation, and a bootstrap confidence interval for the median. Store every raw
sample. A generated summary must be reproducible from raw JSON.

Record:

- operating system, kernel, CPU, logical CPU count, and power policy
- filesystem, mount options, and fixture location
- memory size and current background-load check
- compiler and build profile
- implementation versions, commits, and binary digests
- adapter configuration and declared capabilities

## Gates and Interpretation

Correctness and structural gates are portable:

- output digests match
- an unfiltered provider-order first page of 256 rows examines at most 512
  provider rows on the flat fixture
- sparse filters produce the correct matches and preserve continuation without
  a fixed examined-row ceiling; snapshot-only sorting/grouping stays explicit
- cancellation permits its documented terminal status and rejects stale success
  results after the scenario's cancellation barrier
- mutable views converge without duplicate or missing rows

Performance regression gates run on a reference machine. Begin with:

- median regression greater than 10 percent is a warning
- p95 regression greater than 15 percent is a warning
- the same regression reproduced in two clean runs is a failure

Competitor ratios are informational until the suite has three stable baselines.
After that, record a target per scenario instead of one global score. A useful
initial target is Filer first-page latency within 1.5 times the fastest
semantically equivalent framework adapter.

Do not hide a tradeoff in an aggregate score. Publish browse, search,
presentation, responsiveness, resource, and reliability results separately.

## Result Storage

Store:

- versioned fixture manifests and expected digests
- adapter capability and version records
- raw JSON samples
- generated Markdown summaries
- traces and terminal transcripts only for failed samples

Keep machine-specific baselines in named directories. Do not overwrite old
results when a dependency, fixture, protocol, or machine profile changes.

## Implementation Boundary

Place the comparison runner and adapters in an isolated benchmark package under
`filer-core/benchmarks/`. Keep its dependencies out of Filer-core production
and normal dev dependency graphs.

The 0.3.1 stages are:

1. CORE-030: protocol/schema examples, two flat manifests, and conformance design
2. CORE-039: executable validation and flat fixture generation
3. CORE-040 under CORE-031: isolated runner and Filer public-command adapter
4. CORE-041 under CORE-031: std/Tokio adapters, raw JSON, and generated reports
5. CORE-032: one reference-client browse journey and a recorded virtual-view baseline

CORE-042 later adds the remaining fixture corpus, search, mutation, decoration,
responsiveness, and internal trace attribution. CORE-034 adds GIO, KIO, and
recursive walkers after those fixtures exist. CORE-033 adds Yazi and Broot
independently of the system-framework adapters. All three are deferred without
a release milestone. Record missing capabilities explicitly when these stages
are selected.

## Reference Interfaces

- [GIO FileEnumerator](https://docs.gtk.org/gio/class.FileEnumerator.html)
- [KDE KCoreDirLister](https://api.kde.org/kcoredirlister.html)
- [Yazi](https://github.com/sxyazi/yazi)
- [Broot launch and command interface](https://dystroy.org/broot/launch/)
- [walkdir](https://github.com/BurntSushi/walkdir)
- [jwalk](https://docs.rs/jwalk/latest/jwalk/struct.WalkDirGeneric.html)
- [ignore WalkParallel](https://docs.rs/ignore/latest/ignore/struct.WalkParallel.html)
- [Hyperfine process benchmark runner](https://github.com/sharkdp/hyperfine)
