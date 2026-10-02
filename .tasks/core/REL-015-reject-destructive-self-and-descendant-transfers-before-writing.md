---
id: "REL-015"
title: "Reject destructive self and descendant transfers before writing"
status: Done
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["PROVIDER-ACCESS", "SESSION-BOUNDARY"]
risk: "High"
tags: ["operations", "validation", "bug", "ready-for-agent"]
last_updated: 2026-10-02
---

## Summary

Copying a file into its own parent can truncate the source, and copying a directory into its descendant can recurse into the destination until the process aborts. Add real-provider regressions before extracting shared transfer preflight. Check filesystem identity and ancestry, including supported aliases, before either copy or move changes its destination. Path text alone cannot establish object identity.

## Acceptance Criteria

- [x] Public Copy rejects a file copied into its own parent and onto a hard-link or symlink alias, preserving source and destination bytes.
- [x] Copy and Move reject identical directory targets and descendant destinations, including destinations reached through symlinked ancestors; missing destination suffixes are checked through existing ancestors.
- [x] Rejection reports all supplied Session, Request, and Operation identities and performs no destination creation, overwrite, source removal, or recursive traversal.
- [x] Shared preflight owns the checks, provider limitations fail explicitly, valid transfers still work, and dangerous unfixed reproducers run only in isolated subprocesses.
- [x] Real-provider tests cover supported platforms and aliases without sleep-based ordering, and cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.


## Verification

`crates/filer-core/tests/transfer_preflight_test.rs` exercises public Copy and Move
commands with LocalFs. Ten regressions cover identical targets, hard links,
symlinks, existing descendants, missing suffixes, parent components after missing
directories, batch rejection, provider limitations, and valid transfers. Error
assertions check Session, Request, and Operation correlation. Rejection assertions
check preserved bytes and directory contents. Dangerous cases use subprocesses
with temporary directories and a timeout; ordering uses command events.

The own-parent regression failed before identity preflight. Descendant and
missing-parent-component regressions aborted their isolated child processes with
stack overflows before the ancestry checks passed.

Linux validation on 2026-10-02 passed `cargo fmt --all --check`,
`cargo check -p filer-core`, `cargo test -p filer-core`, and
`cargo clippy -p filer-core --all-targets -- -D warnings`. The core suite reports
918 passed tests and 23 ignored tests. Platform-gated symlink fixtures cover Unix
and Windows; Windows reports a skip when symlink creation privileges are absent.
Windows and macOS execution remain CI validation.

Preflight observes filesystem state without reserving targets against concurrent
changes. OPS-004 owns target rechecks when queued operations start.
