---
id: "TUI-002"
title: "Establish the terminal client crate and browsing model"
status: Done
priority: "High"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "model", "testing", "enhancement", "ready-for-agent"]
last_updated: 2026-10-16
---

## Summary

Terminal rendering must not own asynchronous request correctness. Establish crates/filer-tui with a renderer-independent browsing model, using Location and NodeEntry identities from core. Write external tests in the crate tests directory before implementing transitions; avoid a shared client crate until the desktop or remote adapter demonstrates reusable behavior.

## Acceptance Criteria

- [x] A library test target models the active Session and Request, loaded pages, visible range, selected Locations, pipeline choices, loading state, and recoverable errors without importing Ratatui or TachyonFX in the model.
- [x] Tests cover navigation replacement, stale events, duplicate continuations, selection across same-folder refresh, empty folders, and errors while preserving the last usable view.
- [x] The crate lives under crates/ and follows the root workspace layout; library checks and tests compile independently of terminal initialization.
- [x] Implementation follows existing external test style and module-size guidance; cargo fmt --check and focused crate checks/tests pass.

## Verification

Eight external model tests and one doctest pass with `cargo test --locked -p filer-tui`. Tests exercise request and Session correlation, navigation and pipeline replacement, single-use continuations, page validation, refresh selection, empty folders, snapshots, and recovery before and after loaded results.

`cargo fmt --all --check`, `cargo check --locked -p filer-tui --lib`, and `cargo clippy --locked -p filer-tui --all-targets --no-deps -- -D warnings` pass. The crate depends only on public core contracts with default features disabled. It performs no filesystem or terminal initialization.

Clippy without `--no-deps` reports existing `type_complexity` diagnostics in `crates/filer-core/src/api/module.rs` at lines 58 and 59.
