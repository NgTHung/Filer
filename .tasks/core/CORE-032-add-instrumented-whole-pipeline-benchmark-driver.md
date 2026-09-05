---
id: "CORE-032"
title: "Measure one public-core browse journey"
status: "To Do"
priority: "High"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-031"]
risk: "Medium"
impact: "Measures one correct input-to-view journey without requiring a desktop client."
tags: ["core", "performance", "benchmark", "tracing", "pipeline", "enhancement", "ready-for-agent"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Build a deterministic reference client for one journey: open flat-10k, commit a viewport, continue paging, sort by name, apply and clear a name filter, then refresh. Consume public commands and events and validate each committed virtual view. First land state transitions and their tests, then scripted timing and reporting in a separate commit. Extended search, mutation, decorations, trace attribution, and concurrent responsiveness belong to CORE-042.

## Acceptance Criteria

- [ ] The reference client correlates sessions and requests and validates visible identities and ordering after every action in the initial browse journey.
- [ ] Tests reject stale responses, duplicate pages, wrong view digests, and missing terminal events; the harness cleans up pending work after failed samples.
- [ ] Results record semantic-input-to-correct-virtual-view latency using one monotonic clock, separately from engine and core results; virtual commits are never described as physical rendering.
- [ ] Raw JSON, a generated baseline report, and one documented command reproduce the journey without filer-app changes.
