---
id: "TUI-003"
title: "Connect terminal browsing to embedded core"
status: "To Do"
priority: "High"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
depends_on: ["tui:TUI-002", "core:API-020", "core:REL-014"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "api", "navigation", "testing", "enhancement", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

The terminal client sends intent and consumes core results instead of listing files itself. Connect the browsing model to one embedded FilerCore and one Session through a single asynchronous event consumer. Keep provider work and pipeline transformations in core.

## Acceptance Criteria

- [ ] Handshake binds the Session by Request identity; public Navigate, history/up, Scan/continuation, pipeline changes, Refresh, Watch, and explicit read cancellation drive browsing.
- [ ] The bridge distributes events to the model without competing receiver clones and keeps receiving during loading, error recovery, and read-only shutdown.
- [ ] Public-core integration tests cover rapid folder changes, continuation, external filesystem refresh, permission/disappearing-file errors, and late results with barriers rather than sleeps.
- [ ] All filesystem behavior remains behind core, errors remain visible, and crate checks/tests pass.
