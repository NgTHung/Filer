---
id: "UI-006"
title: "Evaluate Slint for filer-app"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Tests the retained Rust-oriented candidate and its model-backed virtual list against desktop requirements."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build a Slint adapter that exposes the shared controller through a model-backed ListView. Keep Slint markup, generated bindings, and renderer types outside framework-free modules.

## Exit Criteria

- [ ] The Slint model and ListView complete the shared million-entry workload while instantiating only visible delegates.
- [ ] Core event delivery updates models on the UI thread with bounded notifications and without replacing the full row model for each page or change.
- [ ] Windows tests cover multi-item drag-and-drop, clipboard, IME, accessibility roles and table state, keyboard multi-selection, high DPI, and multi-window behavior.
- [ ] The spike records renderer selection, Rust-to-Slint binding volume, generated code implications, packaging, and license obligations.
- [ ] Interaction tests exercise the shared workflows and report any semantic or framework test-runner gaps.
- [ ] Release-build measurements record the exact Slint version, backend, adapter size, missing capabilities, and local patches.
