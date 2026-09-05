---
id: "CORE-036"
title: "Separate scan execution from actor orchestration"
status: "To Do"
priority: "Medium"
type: "Refactor"
parent: "core:CORE-019"
milestone: "0.3.1"
rules: ["ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["core", "refactor", "paging", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Split modules/scan/scanner.rs into actor orchestration and focused cache, paged, full, and segmented execution responsibilities. Reuse existing paging modules rather than introducing a second continuation owner. Move emit helpers first, then extract execution paths in separate commits; keep complex changes below 700 changed lines. Do not change the CORE-021 filtering contract.

## Acceptance Criteria

- [ ] Scanner and extracted modules each remain under 700 lines, with shared progress/result emission and stable public imports.
- [ ] Cache hits, segmented locations, sorted snapshots, streaming pages, stale-result suppression, and cancellation retain existing semantics.
- [ ] Scanner cache, paging, streaming, and public large-directory tests pass, followed by cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core.
