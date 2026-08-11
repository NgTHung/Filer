---
id: "UI-003"
title: "Evaluate egui for filer-app"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Tests the mature immediate-mode Rust candidate against Filer's large-directory and desktop requirements."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build an egui and eframe adapter for the shared evaluation lab. Keep the retained app model outside egui and use documented visible-row APIs rather than constructing the complete directory each frame.

## Exit Criteria

- [ ] The adapter completes every shared interaction scenario with no egui type in framework-free modules.
- [ ] The details view uses visible-row virtualization and proves row construction and formatting remain proportional to the viewport.
- [ ] Core events wake the event loop without continuous repaint or idle polling, and burst delivery remains responsive.
- [ ] Windows tests cover multi-item drag-and-drop, clipboard, IME, keyboard multi-selection, accessibility projection, high DPI, and at least two windows or a recorded limitation.
- [ ] Interaction tests and failure artifacts cover the required workflows without relying only on screenshots.
- [ ] Release-build measurements and a written result record exact egui, eframe, renderer, and backend versions plus all contract failures and local patches.
