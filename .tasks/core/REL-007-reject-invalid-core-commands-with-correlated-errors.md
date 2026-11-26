---
id: "REL-007"
title: "Reject invalid core commands with correlated errors"
status: Done
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:REL-015"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["api", "validation", "errors", "bug", "ready-for-agent"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: 2026-11-26
---

## Summary

Make command rejection observable under ADR-0001. The router currently rejects unknown Sessions but logs and drops commands with no registered handler; value checks are spread across handlers. Inventory public command classes, extract shared validation where it repeats, and test each rejection before changing dispatch. Native input validation is the scope; restricted-Session authorization remains deferred in REL-010.

## Acceptance Criteria

- [x] Unknown Sessions, malformed command values, unresolved or unsupported targets, and unavailable command handlers produce structured failures carrying every supplied correlation identity.
- [x] Rejections never disappear into logging alone and never perform mutation or the rejected filesystem action; required target resolution may still report provider errors.
- [x] Transport/channel submission is distinguished from mutation acceptance so callers cannot interpret a successful send as queue admission.
- [x] Validation rules shared by command classes have one owner; public-interface tests prove representative invalid commands and valid commands take the intended routes.
- [x] Documentation names the supported native permission model without promising restricted policy enforcement; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.

- [x] Create and Rename accept exactly one native filename component; empty names, dot/parent components, rooted names, embedded separators, and platform-invalid names reject before writes. Public tests prove ../escaped.txt cannot escape its parent and valid native names remain supported.
- [x] Transfer preflight from REL-015 is shared by direct command validation and execution; rejection keeps its correlation and never truncates an existing object. OPS-004 owns rechecking targets when queued work starts.
- [x] Land shared native-value validation before router rejection coverage in separate test-first commits; mutation acceptance events are implemented by OPS-004.

## Verification

The public [command validation tests](../../crates/filer-core/tests/command_validation_test.rs)
check rejection correlation, filesystem contents, and valid command routes.
The [transfer tests](../../crates/filer-core/tests/transfer_preflight_test.rs)
check that preflight failures preserve existing files. The
[Core API reference](../../docs/core-api.md#commands) describes channel
submission and native filesystem permissions.

Linux passed `cargo fmt --all --check`, `cargo check -p filer-core`, and
`cargo clippy -p filer-core --all-targets -- -D warnings`.
`cargo test -p filer-core` reports 936 passed and 22 ignored. The command
validation tests compile for `x86_64-pc-windows-gnu` with default features
disabled. Windows runtime coverage runs in CI.
