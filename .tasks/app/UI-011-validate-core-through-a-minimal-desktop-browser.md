---
id: "UI-011"
title: "Validate core through a minimal desktop browser"
status: "To Do"
priority: "High"
type: "Epic"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["testing", "performance", "validation"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Provide a visible consumer of current public Filer-core contracts during 0.3.1. This is the maintainer-approved exception to the core-first focus, independent of deferred UI-001 framework selection. Build one window and one session with folder navigation, loaded-row selection, a virtualized directory list, continuation paging, recoverable errors, and asynchronous Git decorations. The existing Iced dependency is provisional and confined to the renderer. This track provides feedback alongside core work and is not an additional 0.3.1 core release gate.

## Exit Criteria

- [ ] UI-012 through UI-015 are Done: one documented command opens a local folder using current LocationRef/NodeEntry commands and events.
- [ ] A 10,000-entry folder becomes usable before full traversal; rendering work follows the visible range, paging stays correct, and late decorations do not block interaction.
- [ ] Navigation, refresh, selection, stale results, errors, and shutdown are covered by automated state/bridge tests and a recorded real-window smoke run.
- [ ] Core-owned failures found during validation have concrete reproduction/regression tickets; renderer limitations are recorded without treating this client as the final framework decision.
