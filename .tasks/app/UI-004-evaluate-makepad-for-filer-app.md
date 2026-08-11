---
id: "UI-004"
title: "Evaluate Makepad for filer-app"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Tests Makepad's virtualized GPU widget stack while exposing release and desktop integration risks."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build a Makepad adapter around PortalList and the shared controller. Evaluate the released crate separately from features available only on the active development branch, and pin any source revision used by the spike.

## Exit Criteria

- [ ] PortalList completes the shared million-entry workload with visible widget reuse and no work proportional to the logical entry count during scrolling.
- [ ] UiRunner or the current supported signal path delivers core events to the UI thread without busy polling or unsafe state mutation.
- [ ] The spike records whether a released Makepad version can meet the contract; any development revision is pinned and its unreleased dependencies are listed.
- [ ] Windows tests reproduce and assess multi-file drag-and-drop, clipboard, IME, complex text shaping, accessibility-tree exposure, high DPI, and multi-window behavior.
- [ ] Makepad's UI test runner executes the shared workflows on Windows CI or the missing support is reproduced and recorded as a blocker.
- [ ] Release-build measurements record renderer backend, adapter size, required framework patches, documentation gaps, and maintenance risk.
