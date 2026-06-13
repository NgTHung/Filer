# Async/Actor Correctness and Cancellation Review (CORE-008)

Do the filer-core actors cancel superseded work correctly, guard against stale results,
avoid leaking spawned tasks, apply channel backpressure, and shut down cleanly? This report
audits per-session cancellation, stale-result guards, task lifetime, the unbounded flume
channels, and the shutdown path across the six long-running actors. It is review-only and
changes no production code.

Evidence is cited as `path:line` against the crate at the time of review. Test modules under
`src/tests/` are out of scope.

## The actor model in one paragraph

Each long-running concern runs as an `Actor` (`actors/mod.rs:29`) spawned through `ActorSystem`
(`actors/mod.rs:42`), which tracks `JoinHandle`s so `shutdown()` can abort them
(`actors/mod.rs:72`). Actors receive commands and emit events over flume channels. Five actors
(scanner, searcher, previewer, operator, and the router) own per-session cancellation via
`CancelMap` (`actors/cancel.rs:68`): a new command calls `arm(session)` to cancel any prior
in-flight token and register a fresh one (`cancel.rs:82`), the spawned task checks
`is_cancelled()` and `is_latest()` before emitting, and on finish it removes its map entry.
`CancelMap` deliberately exposes two removal methods: `remove` (unconditional, `cancel.rs:99`)
and `remove_if_current` (removes only if the token still matches, `cancel.rs:104`). Choosing the
wrong one is the central defect this audit found.

## Cancellation is armed correctly, but two emit paths are never armed

Every actor that does long work arms a token on each new command. Scanner arms in
`dispatch_scan_with_location` (`scanner.rs:166`), searcher in `dispatch_search_with_location`
(`searcher.rs:99`), previewer in `dispatch_preview` (`previewer.rs:149`), and operator arms per
operation for copy (`operator.rs:253`), move (`operator.rs:446`), and delete
(`operator.rs:585`). The spawned task then checks `is_cancelled()` before starting work and
again after each await before emitting, which is the correct shape.

Two classes of spawned work are never armed, so they cannot be cancelled mid-flight:

- Previewer metadata dispatch spawns at `previewer.rs:349` (`dispatch_metadata`) and
  `previewer.rs:399` (`dispatch_metadata_location`) with no `arm_cancel`. The task is still
  stale-guarded by `mark_latest` (`previewer.rs:331`, `:385`) and an `is_latest` check
  (`previewer.rs:350`, `:400`), so it never emits a superseded result and never inserts a
  map entry to leak. The only cost is wasted work: a slow `FileNode::from_path`
  (`previewer.rs:353`) keeps running after the user navigates away or destroys the session.
  Severity low, because the call is a single bounded metadata read.

- Operator's fast operations spawn without arming: rename (`operator.rs:749`),
  create_file (`operator.rs:841`), and create_folder (`operator.rs:931`). These are intended
  to be near-instant single-syscall operations, so the absence of cancellation is defensible.
  They also never create a map entry, so they do not leak. Worth a one-line doc note that
  these are deliberately non-cancellable, nothing more.

## The large-directory scan path has a non-cancellable CPU loop

This is the axis the non-blocking large-directory proof target depends on. The scan token is
checked at the right points around I/O: in `load_provider` the fallback path checks after the
single listing call (`paging.rs:93`) and the native path checks before and after every provider
page (`paging.rs:101`, `:114`). The native paging path is responsive because the cancellation
check sits inside the page loop.

The fallback provider path is not. `ProviderPaging::Fallback` lists the entire directory in one
await (`paging.rs:90`), and then `PageSelection::extend` (`paging.rs:287`) runs the pipeline and
a sorted insert over every returned entry in a tight loop with no `is_cancelled()` check
(`paging.rs:288-311`). Memory stays bounded because the selection pops back down to the page
limit (`paging.rs:308`), so this is a CPU-bound stall, not a memory blowup. On a directory with
hundreds of thousands of entries served by a fallback provider, a cancel issued mid-processing
is not observed until the loop finishes. The severity depends on which paging mode the local
provider uses; if it is `Fallback`, this directly weakens the proof target. The fix threads the
token into `extend` and checks it every N entries (the same cadence the native path already
uses between pages).

## Stale-result clobber: searcher and previewer use the wrong removal method

All five armed actors guard emission with an `is_latest` check against a per-session
`latest_*` request map, set synchronously at dispatch time. Searcher sets it at
`searcher.rs:97-98` and checks at `searcher.rs:156`, `:182`. Previewer sets it via `mark_latest`
(`previewer.rs:118`) and checks at `previewer.rs:167`. Scanner uses `latest_scans`
(`scanner.rs:164-165`). Because the latest-request map is updated before the old task can emit,
no actor emits a result for a superseded request. Stale-result *emission* is therefore not a
live bug.

The race is in cleanup. When a spawned task finishes it removes its cancel-map entry. Operator
and scanner do this safely with `remove_if_current`, which removes only when the stored token is
still the one this task armed (`operator.rs:400`, `:553`, `:701`; `scanner.rs:186`). Searcher and
previewer instead call the unconditional `remove` (`searcher.rs:115`; `previewer.rs:204`, `:292`,
`:507`, `:584`). The interleaving that breaks:

1. Request R1 arms token T1; map[session] = T1. Task A spawns.
2. Request R2 arrives. `arm` removes and cancels T1, inserts T2; map[session] = T2. Task B spawns.
3. Task A sees it is cancelled, returns, and calls `remove(session)`, deleting **T2** from the map.
4. Map[session] is now empty while Task B is still running with T2.

The consequence is not a stale emission (the `latest` map still blocks that). It is that Task B
is no longer cancellable: a later `arm` finds no entry to cancel, and a session-destroy
`Cancel` (`search/mod.rs`, `preview/mod.rs` destroy hooks) finds nothing to cancel, so Task B
runs to completion as orphaned work. This is exactly the case `remove_if_current` was added to
prevent, and the two slow actors that need it most do not use it. Severity medium: it leaks no
memory and emits no wrong result, but it defeats cancellation under rapid re-issue, which is the
common case for search-as-you-type and preview-on-cursor-move. The fix is mechanical: switch the
four searcher/previewer sites to `remove_if_current(session, &cancel)`, matching operator and
scanner.

## Task leaks: detached tasks are not joined on shutdown

Per-command work runs in detached `tokio::spawn` tasks that `ActorSystem` does not track; only
the actor `run` loops hold tracked `JoinHandle`s (`actors/mod.rs:57-64`). On a clean
per-command lifecycle this is fine, because every armed task removes its own map entry on all
exit paths (success, error, early cancel return). Operator's copy, move, and delete all reach
their `remove_if_current` cleanup on every branch (`operator.rs:400`, `:553`, `:701`), and
scanner reaches `remove_if_current` after the scan (`scanner.rs:186`).

The leak is at shutdown. `ActorSystem::shutdown` aborts the tracked actor loops
(`actors/mod.rs:73`), and each actor's loop calls `cancel_all` on its `CancelMap` when its
channel closes (`scanner.rs:984`, `searcher.rs:275`, `operator.rs:1191`, `previewer.rs:654`).
But `cancel_all` only sets the atomic flag (`cancel.rs:112`); it does not await the detached
tasks. A task blocked in non-cancellable I/O or in the non-cancellable `extend` loop keeps
running after `shutdown()` returns, because nothing tracks or joins it. `FilerCore::shutdown`
(`api/handle.rs:261`) therefore returns before in-flight file operations or scans have actually
stopped. For a long copy this means the process can still be touching the filesystem after the
caller believes shutdown completed. Severity medium. A clean fix tracks per-task handles (a
per-actor `JoinSet`) and awaits them after `cancel_all`, or documents that shutdown is
fire-and-forget and the runtime drop is the real stop.

A minor related point: `ActorSystem` never drains its `handles` vector
(`actors/mod.rs:64`), so finished `JoinHandle`s accumulate for the process lifetime. Bounded by
total spawn count, low severity.

## Navigator delegates cancellation but does not propagate session-destroy

The navigator spawns no tasks of its own; it mutates per-session `NavigatorState` and forwards
`ScanCommand`s to the scanner (`navigator.rs:698`). Cancellation of the actual directory work is
therefore the scanner's job, and the scanner's `arm`/`is_latest` logic handles rapid navigation
correctly. One gap: `NavCommand::RemoveSession` (`navigator.rs:594`) drops the navigator's own
session state but sends no cancel to the scanner, so an in-flight scan for a destroyed session
runs to completion. The scanner's `is_latest` guard and best-effort `send_or_warn` keep this
harmless (no wrong event reaches a live session), but it is wasted work on session teardown.
Severity low. Forwarding a `ScanCommand::Cancel(session)` on `RemoveSession` would close it.

## Channel backpressure: every channel is unbounded

All twelve channels in the crate are `flume::unbounded` (`handle.rs:91-92`;
`navigation/mod.rs:52`; `scan/mod.rs:46`, `:56`; `search/mod.rs:34`; `preview/mod.rs:34`;
`operations/mod.rs:39`, `:49`; `watch/mod.rs:54`; `watcher.rs:92`). There is no backpressure
anywhere. The `send_or_warn` helpers warn only on a closed channel, never on a full one, because
an unbounded channel is never full.

For command channels this is low risk: commands are small and consumed quickly. The real
exposure is event floods from a single producer outrunning a slow or absent consumer:

- A large scan emits a progress event and a page event per page plus per-stage progress
  updates (`scanner.rs` emit sites). Against a slow UI consumer, these queue without bound.
- Operator copy emits a progress event per file during recursive descent
  (`operator.rs:1070`) with no bound on tree size.
- The watcher's internal `change_rx` (`watcher.rs:92`) takes raw filesystem events. A
  high-churn directory can enqueue faster than `dispatch_change` drains, and because the
  watcher processes changes synchronously in its loop, a flood both grows the queue and stalls
  command handling for that actor.

None of these is a guaranteed failure for a local single-consumer file explorer, where the UI
usually drains promptly. But the large-directory proof target is precisely the case where an
absent or paused consumer lets a scan's event backlog grow unbounded in memory. Severity medium
for the scan and watcher paths. The decision is whether to adopt bounded channels with a defined
overflow policy (block the producer, or coalesce progress events) for the high-volume event
streams; that is a design call for CORE-013, not a local patch.

## Watcher is synchronous and correctly session-scoped

The watcher (`watcher.rs`) holds no `CancelMap` and spawns no tasks; it processes filesystem
changes inline in its loop (`watcher.rs:323`). Its events carry session identity correctly:
`dispatch_change` tags each emitted event with the owning `session` from the watch entry
(`watcher.rs:295-307`), satisfying SESSION-BOUNDARY. Session destroy is wired: the watch module
registers an `on_session_destroy` hook that sends `UnwatchSession` (`watch/mod.rs`), and
`handle_unwatch_session` removes the session from every watch and tears down empty watches
(`watcher.rs:250-267`). It needs no `cancel_all` because it owns no tasks. The only watcher
concern is the unbounded synchronous event path noted above.

## Session-destroy cleanup is complete across modules

Every module registers an `on_session_destroy` hook, and the router runs them after the destroy
handler (`router.rs:104-105`): navigation sends `RemoveSession`, watch sends `UnwatchSession`,
and search, operations, and preview each send `Cancel(session)`. So per-session actor state and
cancel-map entries are released on destroy. The one weakness already noted is that the searcher
and previewer `Cancel` may find an empty map entry because of the `remove`-clobber race, in
which case an orphaned in-flight task is not cancelled on destroy. That is the same defect as
the stale-result clobber section, surfacing again at the destroy boundary.

## Per-actor summary

| Actor | Cancellation | Stale guard | Task cleanup | Backpressure | Shutdown |
| --- | --- | --- | --- | --- | --- |
| scanner | armed; fallback `extend` loop not interruptible | `is_latest` + `remove_if_current` (safe) | safe on all paths | unbounded events | `cancel_all`, not joined |
| operator | copy/move/delete armed; rename/create not | `remove_if_current` (safe) | safe on all paths | unbounded; per-file progress | `cancel_all`, not joined |
| searcher | armed | `is_latest` ok; **`remove` clobber** | clobber defeats cancel | unbounded batches | `cancel_all`, not joined |
| previewer | preview/extended armed; metadata not | `is_latest` ok; **`remove` clobber** | clobber defeats cancel; metadata never armed | unbounded | `cancel_all`, not joined |
| navigator | delegates to scanner | n/a (emits snapshots) | spawns nothing | unbounded | implicit; no scanner cancel on destroy |
| watcher | synchronous, no tasks | session-scoped events | n/a | unbounded; synchronous flood stalls loop | implicit |

## Missing cancellation/backpressure test scenarios

These are follow-up test candidates, not findings:

- Cancel a fallback-provider scan of a very large directory and assert it stops promptly;
  today `PageSelection::extend` would run to completion first.
- Rapid re-issue of search and preview for one session, then cancel, asserting the latest
  in-flight task is actually cancelled. The current `remove` clobber would leave it running, so
  this test should fail until the fix lands.
- Shutdown while a long copy or scan is in flight, asserting no filesystem activity continues
  after `FilerCore::shutdown` returns. Exposes the unjoined detached tasks.
- Flood the watcher's `change_rx` faster than `dispatch_change` drains and assert command
  handling is not starved; documents the synchronous-dispatch backpressure behavior.
- Session destroy mid-search/mid-preview, asserting the in-flight task is cancelled (couples to
  the `remove_if_current` fix).

## Follow-up task candidates

Candidates for the CORE-013 remediation backlog, not new tasks created here.

- Switch searcher (`searcher.rs:115`) and previewer (`previewer.rs:204`, `:292`, `:507`, `:584`)
  from `remove` to `remove_if_current`, matching operator and scanner. Severity: Medium. This is
  the one correctness defect; it defeats cancellation on rapid re-issue and on session destroy.
- Thread the cancellation token into `PageSelection::extend` (`paging.rs:287`) and check it
  every N entries so the fallback large-directory scan is interruptible. Severity: Medium for
  the proof target; depends on the local provider's paging mode.
- Join detached per-command tasks on shutdown (per-actor `JoinSet` awaited after `cancel_all`),
  or document shutdown as fire-and-forget. Severity: Medium. Affects whether `FilerCore::shutdown`
  truly quiesces the filesystem.
- Decide a backpressure policy for high-volume event streams (scan pages/progress, watcher
  changes): bounded channels with block-or-coalesce. Severity: Medium; gates the large-directory
  non-blocking target under a slow consumer.
- Forward `ScanCommand::Cancel(session)` from `NavCommand::RemoveSession` (`navigator.rs:594`)
  to stop scans for a destroyed session. Severity: Low.
- Document that operator rename/create (`operator.rs:749`, `:841`, `:931`) and previewer
  metadata dispatch (`previewer.rs:349`, `:399`) are deliberately non-cancellable, or arm them
  for consistency. Severity: Low.
- Drain finished `JoinHandle`s from `ActorSystem.handles` (`actors/mod.rs:64`). Severity: Low.
