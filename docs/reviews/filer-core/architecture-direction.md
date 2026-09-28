# Architecture Direction Review

filer-core is heading in the right direction. Its main contracts hold: Location addressing, bounded event delivery, cancellation that reaches provider I/O, and the accepted runtime lifecycle. The risk is how work is ordered, not how the code is structured. The default sorted folder open costs ten times an unsorted full walk. Meanwhile, benchmark infrastructure and task tooling sit in the queue next to it.

This report is review only. It checks commit `b312737` on 2026-09-24 against `README.md`, `ROADMAP.md`, [ADR 0001](../../adr/0001-core-runtime-lifecycle.md), the 0.3.1 task queue, and the source. Paths are relative to `filer-core/src/` unless stated otherwise. It follows the CORE-013 synthesis in `VERDICT.md` and does not replace it.

## What is working

Location is the only addressing contract. API-008 removed NodeId, and `LocationDescriptor` keeps identity separate from display text. Archives, future providers, and every client can build on that without a second identity table.

The runtime lifecycle is explicit. On shutdown, `WorkTracker` (`actors/mod.rs`) closes admission, cancels outstanding tokens, and joins tracked work. `EventSink` (`api/event_sink.rs`) bounds the event queue and coalesces progress updates per scope, so a slow client cannot grow memory without limit. `ProviderCx::race` (`vfs/context.rs`) drops the provider future when the deadline passes or the cancel signal fires, so a cancel stops the underlying provider call.

Performance claims come with evidence. PIPELINE-003 cut the unsorted first page on 10,000 entries from 98.7 ms to 0.29 ms median. A structural test proves that the first page no longer walks the whole directory. The numbers live in `filer-core/benches/baselines/`.

ADR 0001 settles mutation semantics before code hardens around them. Mutations enter a per-Session FIFO queue that pauses on failure, and SessionDestroyed fires only after accepted work settles.

Scope discipline holds. The remote-provider stubs were removed, and the app rewrite and task-web stay deferred.

## Findings

Findings are ranked by user impact. The owner column names the task that covers the finding, or "Untracked" when no task does.

| # | Finding | Severity | Owner |
| --- | --- | --- | --- |
| D1 | Sorted first page costs ten times the full walk, and name sort compares bytes | High | PIPELINE-008, PIPELINE-002 |
| D2 | Command dispatch converts the typed enum to string keys and back | Medium | API-018, API-019 |
| D3 | Handshake has no request correlation on the shared event stream | Medium | Untracked |
| D4 | Provider trait has four listing entry points and misleading write defaults | Medium | Untracked |
| D5 | Extension types, blocking I/O, and swallowed errors leak across layers | Medium | Partly PREVIEW-001 |
| D6 | Wire contract covers commands but not events or rows | Low | PROTOCOL-001 |
| D7 | Planning and tooling weight competes with product work | Medium | Process |
| D8 | Operator and scanner modules exceed the size limit | Medium | CORE-019, CORE-035, CORE-036 |

### D1. The default browse path is the slow path

The CORE-021 baseline (`filer-core/benches/baselines/2026-09-05-core-021-linux-i7-11800h-btrfs.md`) records these results on 10,000 entries:

| Scenario | Median | Allocations | Allocated bytes |
| --- | ---: | ---: | ---: |
| Sorted first page, 256 rows | 101.1 ms | 509,092 | 33,231,482 |
| Full unsorted snapshot, 10,000 rows | 9.4 ms | 100,380 | 10,200,304 |

The sorted page works out to about 51 allocations and 3.3 KB per directory row. Explorer-style clients open folders sorted, so every folder open pays this cost. The unsorted path that PIPELINE-003 made fast is one that users rarely see.

The sort order is also wrong for an Explorer replacement. `pipeline/order.rs:50` compares names byte by byte, which is case-sensitive and not natural. "Zeta" sorts before "alpha", and "file10" before "file2". Windows Explorer uses logical ordering, so users will read this as a bug. PIPELINE-002 holds natural and locale-aware ordering. The comparator also defines ordered continuations (PIPELINE-006) and cursor stability. Decide the collation before more paging contract work lands on byte order.

All three baselines ran on Linux Btrfs. The stated proof target is `C:\Windows\System32` on NTFS. Commit `6dd1e5c` planned Windows parity work, but no Windows baseline exists yet.

Keep PIPELINE-008 at the head of the core queue. Take the collation decision from PIPELINE-002 right after it. Record an NTFS baseline before you use timing numbers across platforms.

### D2. Typed commands round-trip through strings

`Command` is a typed enum. The router converts it to a string key (`actors/router.rs:111`). `HandlerRegistry::dispatch` allocates that key again (`api/module.rs:119`) and looks up a boxed closure in an `scc::HashMap`. The closure then pattern-matches the same enum. So every command pays two string allocations and a hashed lookup to make a decision the compiler could make with `match`.

The docs still advertise loading modules and swapping handlers at any time (`api/handle.rs:58`, `api/handle.rs:169`, `api/module.rs:21`). ADR 0001 rejects that in favor of startup composition.

API-018 and API-019 own the fix. The recommended end state has built-in commands dispatched with a `match` and no string registry. Compiled-in extensions register typed handles, and the string key survives only as a tracing label. Update the doc comments when API-018 lands.

### D3. Handshake cannot be attributed

`Command::Handshake` carries no `RequestId` (`api/commands.rs:181`). `Event::SessionCreated(SessionId)` (`api/events.rs:126`) goes out on the single shared event stream. When two windows or tabs handshake at the same time, neither can tell which new session is its own.

ADR 0001 accepts one shared stream and has the client bridge distribute events by Session identity. The bridge cannot route a session it cannot attribute. Give `Handshake` a request id and echo it in `SessionCreated`. No task covers this today. The change is small now and becomes a breaking change once multi-window clients exist.

### D4. The provider trait is wider than it needs to be

`FsProvider` (`vfs/provider.rs`) offers four ways to list a directory: `open_listing` (line 90), `list` (line 100), `list_with_options` (line 107), and `list_page` (line 121). The default `list_page` builds the full listing for every page. A provider that forgets to override it pays O(directory) per page, the same cost PIPELINE-003 removed for `LocalFs`.

The default write methods return `permission_denied` (lines 212 to 224). A read-only provider therefore reports "not allowed" when it means "not supported", and clients will show the wrong error. `ErrorKind::Unsupported` and `ErrorCode::UnsupportedOperation` already exist in `errors.rs` for this case.

Before a second real provider lands, make the streaming `open_listing` the one required listing primitive. Derive snapshot and page helpers from it inside core. Return the unsupported error for missing write support.

### D5. Layer leaks

`api/events.rs:9` imports `FileDecoration` and `FileDecorationInvalidation` from `modules::git_decorations`. The public event enum now depends on one extension's types. When MODULES-001 defines the semantic envelope, move decoration types into `model` or the envelope itself.

`NodeEntry::from_path` and related constructors run blocking `std::fs::metadata` inside the model layer (`model/node.rs:74`, `model/node.rs:196`). Model types should not do I/O, because async callers must remember to move each call off the runtime.

`std::fs::read_link(path).unwrap_or_default()` (`model/node.rs:230`, `model/node.rs:249`) swallows the error and reports an empty symlink target. AGENTS.md forbids ignoring errors silently.

Preview providers still open files directly instead of going through the provider. PREVIEW-001 owns that part.

### D6. The wire contract covers one direction

`WireCommand` gives commands a serializable DTO. `Event`, `NodeEntry`, and `GroupedEntries` derive only `Debug` and `Clone`. The CORE-005 claim that the model is transport-ready holds for Location types and commands, not for results.

Deferring this fits the current scope. Keep new result types serde-friendly so PROTOCOL-001 stays a DTO task and does not turn into a redesign.

### D7. Process weight

The task project holds 195 task files, and 22 of the last 50 commits are task commits. `taskroot` has 8,593 source lines and `filer-task-web` has 9,699, together close to filer-core's 19,758 non-test lines.

Milestone 0.3.1 queues five benchmark infrastructure tasks (CORE-030, CORE-031, CORE-039, CORE-040, CORE-041) next to PIPELINE-008. Measurement has paid off, since it exposed D1. But protocol work does not change what you see when you open a folder. Trim the benchmark protocol to what gates a decision. Land the sorted path, the collation decision, and the UI-011 real window first.

### D8. Oversized modules

`modules/operations/operator.rs` has 1,301 lines and `modules/scan/scanner.rs` has 1,082. Both exceed the 1,000-line ceiling in AGENTS.md. CORE-019, CORE-035, and CORE-036 own the split. OPS-004 already depends on CORE-035, so the mutation queue will not land in the largest file in the crate. Keep that dependency when you reorder the queue.

## Recommended order

1. PIPELINE-008, the sorted first-page cost.
2. The collation decision from PIPELINE-002, before more paging contract work.
3. VFS-002 and a Windows NTFS baseline.
4. Handshake correlation, as a new task.
5. API-018, then API-019.
6. CORE-035 and REL-007, then OPS-004 and OPS-005.
7. UI-011 in parallel, as the approved companion track.

## Follow-up candidates

These findings have no owning task. File them with `taskroot` if you accept them.

- Correlate `Handshake` and `SessionCreated` with a request id (D3).
- Narrow `FsProvider` to one streaming listing primitive and return unsupported errors for missing writes (D4).
- Move decoration types out of `modules::git_decorations`, move blocking I/O out of `model/node.rs`, and surface `read_link` failures (D5).
