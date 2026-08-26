---
id: "CORE-032"
title: "Add instrumented whole-pipeline benchmark driver"
status: "To Do"
priority: "High"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-031"]
risk: "Medium"
impact: "Measures browse, presentation, search, cancellation, refresh, and frame commitment as user-visible journeys."
tags: ["core", "performance", "benchmark", "tracing", "pipeline"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Drive semantic application actions through a deterministic reference client and correlate black-box input-to-view latency with Filer trace phases. The reference client consumes public commands and events, maintains application view state, and commits a virtual frame without requiring filer-app work.

## Acceptance Criteria

- [ ] A deterministic reference client consumes public Filer-core commands and events and commits viewport, selection, sort, filter, group, paging, search, and refresh state.
- [ ] Scripted browse-organize-search, mutation-recovery, and decorated-browse journeys define correctness digests at every visible milestone supported by current core capabilities.
- [ ] Correlated trace phases cover input, command routing, provider work, pipeline work, event delivery, view commit, frame commit, and quiescence without changing command semantics.
- [ ] Results report black-box input-to-correct-frame latency beside provider, pipeline, queue, view, and render attribution from one monotonic clock domain.
- [ ] Responsiveness measurements cover input latency, maximum stall, queue high-water mark, stale work, cancellation-to-quiescence, and post-cancel activity while long work runs.
- [ ] Tests reject missing or duplicate trace phases, cross-request correlation, wrong view digests, and terminal work after cancellation.
