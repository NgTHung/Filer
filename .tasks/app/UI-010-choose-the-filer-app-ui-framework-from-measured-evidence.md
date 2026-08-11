---
id: "UI-010"
title: "Choose the filer-app UI framework from measured evidence"
status: "To Do"
priority: "High"
type: "Design"
parent: "UI-001"
depends_on: ["UI-003", "UI-004", "UI-005", "UI-006", "UI-007", "UI-008", "UI-009"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Commits the app rewrite to one evidence-backed framework and records a recoverable fallback."
tags: ["architecture", "benchmark", "performance", "reliability", "dependencies"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Compare the completed candidate results without changing weights after measurements are known. Select one production framework, record a fallback, and translate the decision into staged implementation work.

## Acceptance Criteria

- [ ] A single matrix compares every candidate using the same raw scenarios, environment, required behaviors, performance targets, and maintenance evidence.
- [ ] Mandatory accessibility, input, virtualization, event-loop, testing, and desktop integration failures disqualify a candidate unless an explicit product decision accepts the limitation.
- [ ] The decision record selects one framework and explains measured advantages, accepted costs, version pinning, renderer, packaging, licensing, and supported platforms.
- [ ] The decision record names one fallback and the observable conditions that trigger reconsideration before the deprecated app is removed.
- [ ] A staged implementation epic is created for the selected adapter, beginning with the framework-free controller and one virtualized Location-native directory slice.
- [ ] Rejected spike dependencies and code can be removed without changing the shared architecture or measured result records.
