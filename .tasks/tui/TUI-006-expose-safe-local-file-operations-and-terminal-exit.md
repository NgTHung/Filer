---
id: "TUI-006"
title: "Expose safe local file operations and terminal exit"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
depends_on: ["tui:TUI-005", "core:REL-008", "core:OPS-006"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "operations", "sessions", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

A terminal file manager must preserve accepted work when another command arrives or the user quits. Add copy, move, delete, rename, and create through core after queue admission, failure recovery, and native-work closure are verified. Land command/progress controls and then recovery/exit behavior in separate test-first commits; destructive confirmation and partial outcomes stay visible.

## Acceptance Criteria

- [ ] Commands distinguish submitted, rejected, accepted, queued, running, and terminal outcomes by operation identity; sending a command never displays success by itself.
- [ ] A failed operation shows partial changes and pauses remaining work with explicit retry, continue, and cancel controls; copy-then-delete and cross-device moves have client integration coverage.
- [ ] Quit with accepted work offers finish-and-quit or explicit cancellation, consumes events through cleanup, and interrupts exit on failure to permit recovery.
- [ ] State/bridge and Linux PTY tests cover queued operations, errors, cancellation, terminal restoration, and closure; focused crate/core checks pass.
