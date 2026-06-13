# Rust-Rule and Error-Handling Compliance Review (CORE-007)

Do the filer-core production sources obey the AGENTS.md Rust rules, specifically the ban on
`unwrap`/`expect`, the "no silent error swallowing" rule, and the "avoid unnecessary clone"
rule? This report classifies every production `unwrap`/`expect` site as justified-and-tested
or a violation, lists the silent error-swallowing and clone hotspots with `file:line`, and
proposes follow-up tasks for the violations worth fixing. It is review-only and changes no
production code.

Evidence is cited as `path:line` against the crate at the time of review. Test modules under
`src/tests/` are out of scope; the rules apply to them, but this pass audits production code.

## Unwrap/expect inventory

Eighteen `unwrap`/`expect` calls live in production code. One more (`pipeline/config.rs:44`)
sits inside a `///` doc example and is not production code, so the prior count of nineteen
folds to eighteen real sites.

| Site | Pattern | Classification | Severity |
| --- | --- | --- | --- |
| `actors/mod.rs:64` | `handles.lock().unwrap()` | Justified: std `Mutex` poison-only panic | Low |
| `actors/mod.rs:73` | `handles.lock().unwrap()` | Justified: poison-only | Low |
| `actors/mod.rs:80` | `handles.lock().unwrap()` | Justified: poison-only | Low |
| `api/module.rs:113` | `destroy_hooks.lock().unwrap()` | Justified: poison-only | Low |
| `api/module.rs:130` | `destroy_hooks.lock().unwrap()` | Justified: poison-only | Low |
| `vfs/local_watch.rs:37` | `debouncer.lock().unwrap()` | Justified: poison-only (dead code, `#[allow(unused)]`) | Low |
| `vfs/local_watch.rs:85` | `debouncer.lock().unwrap()` | Justified: poison-only | Low |
| `vfs/local_watch.rs:86` | `guard.as_mut().unwrap()` | Justified-by-construction, untested invariant | Low |
| `vfs/local_watch.rs:96` | `debouncer.lock().unwrap()` | Justified: poison-only | Low |
| `modules/operations/mod.rs:61` | `ops_rx.take().expect(...)` | Justified-by-construction; wrong message text | Low |
| `modules/navigation/mod.rs:70` | `nav_rx.take().expect(...)` | Justified-by-construction | Low |
| `modules/scan/mod.rs:76` | `scan_rx.take().expect(...)` | Justified-by-construction | Low |
| `modules/navigation/navigator.rs:438` | `v.back(1).unwrap()` | Justified-by-guard (`can_back()`) | Low |
| `modules/navigation/navigator.rs:465` | `v.forward().unwrap()` | Justified-by-guard (`can_forward()`) | Low |
| `model/node.rs:83` | `path.strip_prefix("~").unwrap()` | Justified-by-guard (`starts_with("~")`) | Low |
| `services/preview/providers/code.rs:61` | `ts.themes.values().next().unwrap()` | Justified-by-library-invariant, undocumented | Low-Med |
| `modules/watch/watcher.rs:303` | `entry.node.expect(...)` | Violation candidate: unproven invariant | Medium |
| `model/node.rs:414` | `path.to_str().unwrap()` | Violation: panics on non-UTF-8 paths | High |

## Lock-poisoning unwraps are the idiom, not a defect

Eight of the eighteen sites are `std::sync::Mutex::lock().unwrap()`
(`actors/mod.rs:64`, `:73`, `:80`; `api/module.rs:113`, `:130`;
`vfs/local_watch.rs:37`, `:85`, `:96`). `lock()` returns `Err` only when the mutex is
poisoned, which happens only if another thread already panicked while holding the lock. At
that point the program is in an undefined state, so propagating the panic is the defensible
reliability choice. This is the standard Rust idiom.

These technically breach the literal "no `unwrap`" rule, but each is a poison-only path. The
rule's intent is to forbid panics on recoverable conditions, and a poisoned lock is not
recoverable here. Treat them as accepted, with one optional hardening: swapping `std::Mutex`
for a non-poisoning lock (`parking_lot::Mutex`) would remove the `unwrap` entirely and the
rule violation with it. That is a dependency decision for CORE-008, not a correctness fix.

`vfs/local_watch.rs` carries a separate caveat: the whole module is `#[allow(unused)]`
(`vfs/local_watch.rs:28`, `:34`, `:107`), so it is dead code today. Its
`guard.as_mut().unwrap()` (`vfs/local_watch.rs:86`) relies on `ensure_debouncer`
(`vfs/local_watch.rs:36`) having set the `Option` to `Some` before the re-lock. Nothing ever
clears it back to `None`, so the invariant holds, but it spans two separate lock acquisitions
and is untested. Low severity while the module stays unused; revisit if it is wired in.

## Module-init expects are sound but one message is wrong

`modules/operations/mod.rs:61`, `modules/navigation/mod.rs:70`, and `modules/scan/mod.rs:76`
each call `self.<field>_rx.take().expect(...)` inside `Module::init`. The receiver is `Some`
by construction at build time and `init` runs once, so the `Option` is always `Some` here.
These are justified-by-construction.

One defect rides along: `modules/operations/mod.rs:61` uses the message
`"ScanModule already initialized"` inside the operations module. It is a copy-paste error from
the scan module. It never fires in practice, but if it ever did, the panic would name the
wrong module and mislead debugging. Fix the string.

## Guard-protected unwraps are correct; verify the tests cover them

Three sites unwrap a value that a preceding guard guarantees:

- `navigator.rs:438` calls `v.back(1).unwrap()` only inside `if v.can_back()`
  (`navigator.rs:437`).
- `navigator.rs:465` calls `v.forward().unwrap()` only inside `if v.can_forward()`
  (`navigator.rs:464`).
- `node.rs:83` calls `path.strip_prefix("~").unwrap()` only inside
  `if path.starts_with("~")` (`node.rs:81`).

Each unwrap is logically safe. The AGENTS rule requires that exceptions be "validated/tested
fully," so the obligation is to confirm the guard and the unwrapped path are exercised by
tests (`navigator_test.rs` for back/forward, the node tests for `~` expansion). The cleaner
long-term shape for the navigator pair is to fold the guard and the call into one method that
returns a `Result`, removing the unwrap; that is a small refactor, not a bug.

## code.rs theme fallback leans on an undocumented library invariant

`services/preview/providers/code.rs:61` ends a fallback chain with
`.unwrap_or_else(|| ts.themes.values().next().unwrap())`. The final `unwrap` assumes
`ThemeSet::load_defaults()` (`code.rs:51`) always returns a non-empty theme map. That holds
for syntect today, but it is an undocumented dependency on library internals, and the panic
would be a poor failure mode for a preview render. Low-to-medium: the safer form returns a
plain unstyled string when no theme is available, matching the line-level fallback already
used at `code.rs:69`.

## node.rs:414 is a real violation that panics on non-UTF-8 paths

`NodeId::from_path` (`node.rs:409`) hashes a path with
`h.write(path.to_str().unwrap().as_bytes())` (`node.rs:414`). `Path::to_str` returns `None`
for any path that is not valid UTF-8. Such paths exist on both target platforms: Windows paths
are UTF-16 and can hold unpaired surrogates, and Linux paths are arbitrary bytes. A single
such file in a scanned directory panics the hashing of its node ID.

This is the highest-severity finding. `NodeId::from_path` is central: every node gets an ID
through it, so the panic sits on the main scan path, not an edge feature. The fix hashes the
raw OS bytes instead of the UTF-8 view, for example `path.as_os_str()` fed through its `Hash`
impl, or `OsStr::as_encoded_bytes()`. The change is small but needs a test with a non-UTF-8
path (or an `OsStr` built from invalid bytes) to lock the behavior in.

## Silent error swallowing: one real inconsistency, the rest acceptable

The crate already provides the intended non-silent send helper. `utils/channel.rs:3` defines
`send_or_warn` precisely to "replace the `let _ = tx.send(val)` pattern that silently discards
send failures," and the actors that emit events use it (for example `navigator.rs:446`,
`previewer.rs` via `send_or_warn_async`). The rule is established in the codebase itself.

The inconsistency is that the per-module command dispatchers have not adopted it. They still
silently drop sends with `let _ = tx.send(...)`:

- `modules/operations/mod.rs:72`, `:92`, `:112`, `:132`, `:152`, `:172`, `:192`, `:212`,
  `:233`, `:253`, `:274`, `:294`, `:307`, `:313`
- `modules/watch/mod.rs:59`, `:71`, `:83`, `:90`, `:97`, `:103`
- `modules/search/mod.rs:47`, `:70`, `:93`, `:108`, `:114`
- `vfs/local_watch.rs:51` (inside the dead `#[allow(unused)]` module)

These forward a user command to the owning actor. If the actor channel is closed, the command
vanishes with no log and no error returned to the caller, which is exactly the case
`send_or_warn` exists to catch. Severity medium: routing these through `send_or_warn` restores
a diagnostic trail with no behavior change on the happy path.

The remaining `let _ =` sites are not violations:

- `let _ = map.insert_sync(...)` / `remove_sync(...)` on `scc` maps
  (`actors/cancel.rs:87`, `:100`; `api/session_manager.rs:98`, `:111`; `api/module.rs:101`,
  `:102`; `model/registry.rs:40`, `:42`, `:55`; `operator.rs:191`, `:192`, `:203`, `:209`;
  `scanner.rs:164`, `:165`; `searcher.rs:97`, `:98`; `navigator.rs:347`, `:363`, `:413`,
  `:595`) discard the returned previous value, not an error. Ignoring it is correct.
- `metadata.modified()/created()/accessed().ok()` (`node.rs:129-131`, `:213-215`) maps
  unsupported-on-platform timestamps to `None`. This is intentional and the `Option` is
  carried forward honestly.

A few low-severity lossy conversions are worth a glance but not a fix on their own:

- `watcher.rs:186` `register_location_node(location).ok()` drops the registration error and
  proceeds with `node = None`. Downstream handles `None`, but the failure reason is lost.
- `node.rs:286` `read_link(&path).unwrap_or_default()` turns a failed symlink read into an
  empty `PathBuf` silently.
- `utils/size.rs:25-52` and `utils/time.rs:14`, `:51`, `:82`, `:185` map parse failures to
  `None` or a fallback display string. Acceptable for formatting, but the `Err(_)` arms
  discard the cause.

## Needless clones: the rule is largely respected

Production code holds 313 `.clone()` calls, concentrated in `operator.rs` (52),
`previewer.rs` (28), `navigator.rs` (25), and `scanner.rs` (23). The raw count looks alarming,
but it does not indicate a violation of the "avoid unnecessary clone" rule.

The dominant pattern is cheap handle cloning into spawned `'static` tasks. The actor fields
are `Sender<Event>`, `Arc<dyn FsProvider>`, `NodeRegistry`, `Arc<scc::HashMap<...>>`, and
`Option<SharedDirCache>` (`operator.rs:125-133`). Every operation handler clones these into a
`tokio::spawn` closure that must own `'static` data (`operator.rs:254-259`,
`previewer.rs:150-154`). These are `Arc`/`Sender` clones, which copy a pointer and bump a
refcount. They are required by the spawn boundary and are the idiomatic way to share actor
state with a task. They are not needless.

The genuine owned-data clones are few and defensible. `preview.clone()`
(`previewer.rs:175`, `:263`) stores a copy in the cache before emitting the original; the
cache needs ownership and so does the event, so a clone or an `Arc<Preview>` redesign is
required either way. `src_path.clone()` (`operator.rs:291`) hands an owned path to a spawned
task. No high-value needless-clone hotspot surfaced.

Conclusion: no clone violation worth a dedicated fix. If a future pass wants to trim the
`preview.clone()` copies, wrapping the cached preview in `Arc` is the lever, and that belongs
with the `previewer.rs` dedup work already noted under CORE-006, not here.

## Result + ? usage

Spot-checking the fallible production paths shows `Result + ?` used as the rule intends:
`FileNode::from_path` propagates canonicalization errors with `?` and `map_err`
(`node.rs:87`, `:92`), and the watch provider maps and propagates `notify` errors rather than
swallowing them (`local_watch.rs:64-69`, `:88-90`, `:98-100`). No `Result`-returning call was
found being discarded on a path where the error matters, beyond the lossy conversions listed
above.

## Follow-up task candidates

These are candidates for the CORE-013 remediation backlog, not new tasks created here.

- Fix `NodeId::from_path` to hash the OS path bytes instead of `to_str().unwrap()`, with a
  non-UTF-8 path test. Severity: High. This is the one correctness bug in the audit.
- Route the command-dispatcher sends in `operations/mod.rs`, `watch/mod.rs`, and
  `search/mod.rs` through `send_or_warn` to stop silently dropping commands. Severity: Medium.
- Correct the wrong panic message at `operations/mod.rs:61` ("ScanModule" inside the
  operations module). Severity: Low. Trivial, can ride with the send-helper change.
- Replace the `code.rs:61` theme `unwrap` with a graceful unstyled fallback. Severity:
  Low-Med.
- Confirm tests cover the guard-protected unwraps (`navigator.rs:438`, `:465`; `node.rs:83`),
  and consider folding the navigator guard-plus-call into one `Result`-returning method.
  Severity: Low.
- Optional: adopt a non-poisoning mutex to remove the eight lock `unwrap`s wholesale.
  Severity: Low. Dependency decision; pairs with CORE-008.
