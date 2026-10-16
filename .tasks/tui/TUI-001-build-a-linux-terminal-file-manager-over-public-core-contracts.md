---
id: "TUI-001"
title: "Build a Linux terminal file manager over public core contracts"
status: "To Do"
priority: "High"
type: "Epic"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["cli", "local", "validation", "enhancement", "needs-triage"]
last_updated: "2026-10-16"
---

## Summary

Linux users need keyboard-driven file browsing without a desktop window. Build an embedded-core terminal client using Ratatui for rendering and TachyonFX for restrained effects, with one browsing context and one Session. The [terminal interface design](../../docs/tui-design.md) defines the column browser and contextual inspector.

## Delivery scope

TUI-002 through TUI-005 deliver the initial one-pane browsing stage during 0.3.1. Apply the design's sparse list presentation and renderer-independent state updates to this stage. TUI-006 adds local mutations only after core safety and closure contracts land.

Hierarchy columns, contextual child listings, preview, details, and the expanded reader are follow-up design scope. They do not enlarge the existing child tasks or gate their completion. Refine follow-up tasks and their public-core request requirements before implementing these interactions. Independent source/destination panes, remote TUI connections, and extension hosting remain outside this track.

## Exit Criteria

- [ ] TUI-002 through TUI-005 are Done and one documented command browses a local folder with paging, sorting, hidden-file control, selection, refresh, recoverable errors, and correct terminal restoration.
- [ ] The Ratatui and TachyonFX renderer stays separate from tested client state and the core bridge; effects never block input or core event consumption.
- [ ] TUI-006 is Done and accepted mutations expose correlated progress, failure recovery, and explicit finish-or-cancel exit behavior.
- [ ] Core failures found through the client receive finite regression tasks; browsing and mutation readiness are reported separately.
