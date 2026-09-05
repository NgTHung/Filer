---
id: "UI-002"
title: "Establish the filer-app UI evaluation lab"
status: "To Do"
priority: "High"
type: "Epic"
parent: "UI-001"
depends_on: ["core:API-004"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Makes framework results comparable and proves the app controller remains independent of renderer choice."
tags: ["architecture", "testing", "benchmark", "performance", "workspace"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Build the shared, framework-free model, scripted workloads, synthetic core event source, and measurement format used by every UI candidate. The lab starts after API-004 removes compatibility identity so no spike grows around deprecated NodeId contracts.

The active UI-011 validation client is a smaller independent consumer and does not wait for this lab. When evaluation resumes, inspect its framework-free state and tests for reuse, then extend coverage to the full candidate workload. Validation-client results alone cannot satisfy the framework evaluation criteria.

## Exit Criteria

- [ ] A framework-free controller and model fixture exercises Location-native sessions, navigation, paged directories, selection, search, preview, progress, operations, errors, and stale-result rejection.
- [ ] Deterministic datasets cover 100, 10,000, 100,000, and 1,000,000 logical entries without requiring a real filesystem scan.
- [ ] Shared scripts define navigation, page append, rapid scroll, sort and filter replacement, multi-selection, inline rename, search, preview arrival, operation progress, drag-and-drop, IME input, scale change, and error recovery.
- [ ] The result schema records raw frame and input samples, startup, memory, idle wakeups, binary size, build time, exact dependency revision, build profile, operating system, display scale, and hardware.
- [ ] The lab detects full row materialization, full-array frame clones, duplicate page requests, stale-result acceptance, missing terminal events, and busy idle polling.
- [ ] Candidate adapters can be built and removed independently without importing UI framework types into shared model, controller, persistence, or core-bridge contracts.
- [ ] Tests prove the lab produces repeatable results before any candidate result is accepted.
