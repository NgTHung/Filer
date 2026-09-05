---
id: "CORE-031"
title: "Add Filer and Rust primitive benchmark adapters"
status: "To Do"
priority: "High"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-039"]
risk: "Medium"
impact: "Establishes low-level and public-command cost before system frameworks and whole applications are added."
tags: ["core", "performance", "benchmark", "provider", "testing"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Establish the first comparison through CORE-040 (isolated runner and Filer adapter), then CORE-041 (std::fs/Tokio adapters and reports). Consume CORE-039 protocol validation and flat fixtures. Keep benchmark dependencies under filer-core/benchmarks and outside production and normal development builds.

## Acceptance Criteria

- [ ] CORE-040 and CORE-041 are Done.
- [ ] Filer public-command, std::fs, and Tokio adapters cover matching fast and metadata flat-listing scenarios with validated rows and explicit unavailable metrics.
- [ ] Raw samples and generated reports record reproducible baselines with separate engine and core results.
- [ ] The isolated package tests pass without adding benchmark dependencies to production or normal development builds.
