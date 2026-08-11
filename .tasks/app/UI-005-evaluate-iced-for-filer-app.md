---
id: "UI-005"
title: "Evaluate Iced for filer-app"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Rechecks Iced through a clean architecture instead of treating the deprecated filer-app implementation as evidence."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build a new Iced adapter for the shared evaluation lab. Do not repair or copy the deprecated filer-app state model; only the clean controller, Location-native core bridge, and common scenarios count.

## Exit Criteria

- [ ] A clean adapter completes every shared scenario without importing legacy filer-app messages, views, NodeId assumptions, or row construction patterns.
- [ ] The details view proves true visible-row virtualization or records the absence of a maintainable implementation as a failing result.
- [ ] Subscriptions, tasks, or the current supported runtime bridge deliver core events with bounded wakeups, cancellation, and no UI-thread blocking.
- [ ] Windows tests cover multi-item drag-and-drop, clipboard, IME, accessibility, keyboard multi-selection, high DPI, and multi-window behavior.
- [ ] Interaction tests cover the shared behavior and make state transitions observable without screenshot-only assertions.
- [ ] Release-build measurements identify the exact Iced renderer and runtime features, adapter complexity, API stability risks, and any framework patches.
