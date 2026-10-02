---
id: "TUI-005"
title: "Verify terminal browsing readiness on Linux"
status: "To Do"
priority: "High"
type: "Feature"
parent: "tui:TUI-001"
milestone: "0.3.1"
depends_on: ["tui:TUI-004", "core:CORE-045", "core:VFS-003", "core:PIPELINE-011", "core:CORE-046", "core:REL-009"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "performance", "testing", "enhancement", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

A compiling terminal renderer does not prove that large folders, late results, and native filenames are usable. Exercise the complete read-only client on Linux with deterministic state tests, PTY checks, and a recorded interactive smoke run. Keep physical terminal rendering evidence distinct from CORE-032 virtual-view measurements.

## Acceptance Criteria

- [ ] A 10,000-entry fixture supports input during loading; the streaming order shows the first page before full traversal, and the default sorted order is measured without claiming that global sorting avoids traversal.
- [ ] Tests and the smoke run cover paging exactly once, rapid navigation, filter/sort changes, hidden and non-UTF-8 names, symlinked ancestors, external changes, empty folders, recoverable errors, and exit.
- [ ] Selection and row identity remain correct across metadata and refresh; effects enabled and disabled preserve the same browsing results, with idle and active rendering costs recorded on a named machine.
- [ ] Any core-owned failure has a reproducible regression task; focused TUI/core tests and applicable workspace checks pass.
