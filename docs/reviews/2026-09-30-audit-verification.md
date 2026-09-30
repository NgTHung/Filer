# Audit verification, 2026-09-30

File operations need to preserve source data, and completion signals need to mean work has settled. This review independently checks the existing [project audit](2026-09-30-project-audit.md) against the same revision and records additional failures in directory transfer and actor error reporting. It gives remediation owners fresh reproduction evidence without changing production code or task state.

## Scope

Reviewed revision: `6044bfe4404e08120492167655478100c48c1cac`.
Checks ran on Linux with Rust 1.98.1. Core behavior takes priority; supported
workspace crates and the documented desktop build exception also receive checks.
The pre-existing, untracked project audit and skill files remain unchanged.

## Independently confirmed findings

All eight findings in the [project audit](2026-09-30-project-audit.md#reproduced-defects)
remain reproducible. Their detailed source evidence and task ownership stay in
that document.

| Finding | Fresh result |
|---|---|
| F1, High | Same-folder copy and hard-link copy both truncate a 13-byte source to zero bytes and report success. |
| F2, High | Descendant directory copy aborts in an isolated subprocess with stack overflow. |
| F3, High | With an active receiver, 600 handshakes deadlock a single-thread executor; an external four-second timeout returns 124. |
| F4, High | Shutdown returns before a queued native copy starts; releasing the blocked filesystem worker creates the destination afterward. |
| F5, Medium | A size-filtered fast snapshot returns zero rows while a page returns the matching 100-byte file. |
| F6, Medium | A directory move across `/tmp` and `/dev/shm` returns a file-copy error. |
| F7, Medium | Opening an explicit empty ZIP directory returns `PathNotFound`. |
| F8, Medium | A one-byte directory cache retains 519 accounted bytes; a one-byte preview cache retains eight bytes of text. |

## Transfer depth extends F2

A valid, acyclic tree with 256 nested directories also aborts with stack overflow
when copied. The isolated subprocess returns `SIGABRT`. The recursive walk in
[transfer.rs](../../crates/filer-core/src/modules/operations/transfer.rs#L422)
therefore fails on supported input as well as on a self-containing destination.

Rejecting descendant destinations addresses the recursive destination defect,
but it does not make deep source trees safe. Replace recursive traversal with an
explicit work stack, or reject unsupported depths with a correlated error before
mutation. Add separate regressions for descendant rejection and deep-tree
transfer. This extends F2 rather than adding another finding count.

## Actor failure can disappear from shutdown

Medium: [ActorSystem::spawn](../../crates/filer-core/src/actors/mod.rs#L261)
discards finished actor handles before observing their results. If an actor
panics and another actor is then registered, shutdown cannot report the earlier
failure.

A temporary public-API probe loads a module whose actor deliberately panics.
Shutdown returns `ActorFailed`. Repeating the probe and loading another module
with a no-op actor before shutdown changes the result to `Ok(())`. The panic
appears on stderr in both runs, but the caller loses its structured failure
signal in the second run.

Observe completed handles before retiring them, and retain actor failures until
shutdown reports them. A regression should vary whether another actor starts
after the failure. This is an error-observation defect under the repository's
rule against silently ignored errors, independent of planned startup-only
composition.

## Verification

| Check | Result |
|---|---|
| Taskroot validation | 200 tasks, zero warnings |
| `cargo test --locked --workspace --exclude filer-app` | 1,192 passed, 23 ignored, including doctests |
| Benchmark crate tests | 75 passed |
| Workspace Clippy, excluding `filer-app`, with warnings denied | Passed |
| Workspace formatting | Passed |
| Feature checker, minimal and all cases | All five phases passed for both cases |
| `cargo check --locked -p filer-app` | Failed with 29 errors, matching the documented API migration exception |

Temporary probes use public commands or public core composition interfaces.
Deadlock and stack-overflow probes run in subprocesses. No production code,
existing report, or task lifecycle state changed.

Dependency advisory scanning, Windows behavior, desktop interaction, and fresh
comparative performance measurements are outside this verification. Individual
feature combinations beyond minimal and all were not checked.
