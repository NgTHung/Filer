---
id: "UI-007"
title: "Evaluate Qt Quick and CXX-Qt for filer-app"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Measures the mature retained desktop option together with its foreign-language, deployment, and licensing costs."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build a Qt Quick adapter through CXX-Qt, using a model-backed virtual ListView or TableView. Treat QML, generated C++, deployment, and licensing as part of the result rather than external setup.

## Exit Criteria

- [ ] A QAbstractItemModel-compatible bridge completes the shared million-entry workload with delegate virtualization and bounded change notifications.
- [ ] The Rust, CXX-Qt, generated C++, and QML boundary is documented and contains no filer-core behavior or duplicated controller policy.
- [ ] Windows tests cover native multi-item drag-and-drop, clipboard, IME, accessibility table semantics, keyboard multi-selection, high DPI, menus, dialogs, and multiple windows.
- [ ] Interaction tests and diagnostics cover the shared workflows across the Rust and Qt boundary.
- [ ] The report records Qt modules, rendering API, SDK and C++ toolchain setup, deployment size, build time, license obligations, and debugging costs.
- [ ] Release-build measurements record exact Qt and CXX-Qt versions, adapter size, missing capabilities, and local patches.
