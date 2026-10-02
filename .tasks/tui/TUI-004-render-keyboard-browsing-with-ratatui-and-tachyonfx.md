---
id: "TUI-004"
title: "Render keyboard browsing with Ratatui and TachyonFX"
status: "To Do"
priority: "High"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
depends_on: ["tui:TUI-003"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "performance", "validation", "enhancement", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

A Linux terminal client needs readable rows and immediate keyboard feedback while core loads pages. Use [Ratatui](https://ratatui.rs/) for the one-pane layout and [TachyonFX](https://github.com/ratatui/tachyonfx) for restrained feedback effects. Confine both dependencies to terminal presentation; effects operate on rendered output and never become a prerequisite for data or input transitions.

## Acceptance Criteria

- [ ] One documented command opens a folder; keyboard navigation, enter/up/back/forward, refresh, sorting, hidden-file control, and loaded-row selection work in one pane with readable loading/error status.
- [ ] Render work follows the visible terminal range, handles narrow terminals, Unicode display width and resize, and requests pages near the loaded range without sorting rows in the client.
- [ ] TachyonFX supplies bounded feedback effects with a documented disable option; a controllable clock proves input and core results are processed during effects, and navigation/resize cancels obsolete effects.
- [ ] Idle rendering waits for input, core events, or active effect deadlines; exit and recoverable failures restore raw mode, cursor, and alternate-screen state.
- [ ] Ratatui test-backend checks cover layout and effects-disabled output; Linux PTY tests cover resize, exit/error restoration, and crate checks/tests pass.
