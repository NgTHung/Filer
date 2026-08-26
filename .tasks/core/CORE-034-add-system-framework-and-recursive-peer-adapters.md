---
id: "CORE-034"
title: "Add system framework and recursive peer adapters"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-031"]
risk: "Medium"
impact: "Extends semantic comparisons to GIO, KIO, walkdir, jwalk, and ignore without making system libraries required for Filer-core."
tags: ["core", "performance", "benchmark", "provider", "search"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Add capability-gated framework and recursive-search adapters after the common runner and Rust baselines are stable. System packages remain optional, but missing capabilities must be visible in the result set.

## Acceptance Criteria

- [ ] GIO `GFileEnumerator` and KDE `KCoreDirLister` adapters match requested fast and metadata fields and preserve their incremental result boundaries.
- [ ] `walkdir`, `jwalk`, and `ignore::WalkParallel` adapters cover matching recursive search scenarios on multi-directory fixtures.
- [ ] Every adapter records its library or framework version, build options, runtime capabilities, and system dependency availability.
- [ ] Canonical match and row digests gate timing results, including hidden and ignore-aware scenarios.
- [ ] Missing system frameworks produce explicit capability records and do not fail unrelated benchmark adapters.
