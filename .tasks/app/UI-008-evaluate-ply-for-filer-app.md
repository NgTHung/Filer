---
id: "UI-008"
title: "Evaluate Ply for filer-app"
status: "To Do"
priority: "Low"
type: "Epic"
parent: "UI-001"
depends_on: ["UI-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "High"
impact: "Tests whether Ply's small GPU-first stack can satisfy file-manager virtualization and desktop integration contracts."
tags: ["testing", "benchmark", "performance", "dependencies", "portability"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-08-11"
---

## Summary

Build the smallest honest Ply adapter that can attempt the shared scenarios. Rendering culling does not count as list virtualization; the spike must measure row construction and layout cost directly.

## Exit Criteria

- [ ] The spike proves a maintainable visible-row virtualization strategy or records total-row construction or layout as a failing blocker before further polish.
- [ ] If the virtualization gate passes, the adapter completes the shared core-event, Windows integration, accessibility, input, and interaction-test scenarios; otherwise the report marks them unmeasured and explains why more adapter work cannot change the failed result.
- [ ] The report separates Ply behavior from Macroquad and miniquad limitations and records maintainer, release, API stability, and packaging risks.
- [ ] Release-build measurements for every reachable scenario record the exact Ply and backend versions, adapter size, missing capabilities, and local patches.
