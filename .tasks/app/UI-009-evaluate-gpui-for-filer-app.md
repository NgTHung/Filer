---
id: "UI-009"
title: "Evaluate GPUI for filer-app"
status: "To Do"
priority: "Low"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Tests the IDE-proven hybrid Rust framework while making Windows and external-framework support explicit gates."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Verify external GPUI use and Windows support before implementing the full shared workload. Zed product support does not by itself prove a stable public GPUI contract for another Windows application.

## Exit Criteria

- [ ] An independently versioned or pinned GPUI application builds and runs on supported Filer desktop targets, or the exact platform or public-API blocker is reproduced and recorded.
- [ ] If the platform and public-API gate passes, the large-list primitive completes the shared million-entry workload with visible element construction and stable retained application identity; otherwise the report marks the workload unmeasured.
- [ ] If the gate passes, GPUI executors, Windows integration, accessibility, input, and interaction testing complete the shared scenarios without controller policy inside entities or views; otherwise the report records why those scenarios cannot run.
- [ ] Measurements for every reachable scenario record the exact GPUI revision, adapter size, missing capabilities, local patches, pre-1.0 API cost, reliance on Zed-source examples, and upstream maintenance risk.
