---
id: "UI-001"
title: "Select the filer-app UI framework"
status: "To Do"
priority: "High"
type: "Epic"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Chooses the long-lived desktop UI stack and prevents framework concerns from entering filer-core."
tags: ["architecture", "testing", "benchmark", "performance", "dependencies"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Evaluate every serious desktop UI candidate against the same framework-neutral architecture, workload, platform requirements, and measurements. This epic is milestone-free because framework selection must not become an exit gate for the current filer-core roadmap.

## Exit Criteria

- [ ] The shared evaluation lab in UI-002 is complete and every candidate consumes the same framework-free scenarios and result schema.
- [ ] The egui, Makepad, Iced, Slint, Qt Quick, Ply, and GPUI evaluation epics are complete or have a reproducible platform blocker recorded under their own exit criteria.
- [ ] UI-010 records the selected framework, measured evidence, accepted limitations, dependency strategy, and fallback.
- [ ] No candidate receives a passing result through missing behavior, reduced workloads, disabled accessibility, or framework-specific business logic.
- [ ] Prototype dependencies and code are isolated so rejected candidates can be removed without changing the framework-free app model.
