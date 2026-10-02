---
id: "TUI-002"
title: "Establish the terminal client crate and browsing model"
status: "To Do"
priority: "High"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "model", "testing", "enhancement", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

Terminal rendering must not own asynchronous request correctness. Establish crates/filer-tui with a renderer-independent browsing model, using Location and NodeEntry identities from core. Write external tests in the crate tests directory before implementing transitions; avoid a shared client crate until the desktop or remote adapter demonstrates reusable behavior.

## Acceptance Criteria

- [ ] A library test target models the active Session and Request, loaded pages, visible range, selected Locations, pipeline choices, loading state, and recoverable errors without importing Ratatui or TachyonFX in the model.
- [ ] Tests cover navigation replacement, stale events, duplicate continuations, selection across same-folder refresh, empty folders, and errors while preserving the last usable view.
- [ ] The crate lives under crates/ and follows the root workspace layout; library checks and tests compile independently of terminal initialization.
- [ ] Implementation follows existing external test style and module-size guidance; cargo fmt --check and focused crate checks/tests pass.
