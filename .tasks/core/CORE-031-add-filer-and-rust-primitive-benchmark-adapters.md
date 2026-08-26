---
id: "CORE-031"
title: "Add Filer and Rust primitive benchmark adapters"
status: "To Do"
priority: "High"
type: "Feature"
parent: "CORE-029"
milestone: "0.3.1"
depends_on: ["CORE-030"]
risk: "Medium"
impact: "Establishes low-level and public-command cost before system frameworks and whole applications are added."
tags: ["core", "performance", "benchmark", "provider", "testing"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Implement the common comparison runner plus Filer public-command, `std::fs`, and Tokio adapters. Keep the package isolated under `filer-core/benchmarks/` so benchmark dependencies do not enter production or normal development builds.

## Acceptance Criteria

- [ ] The isolated runner executes protocol-conforming adapters and stores raw JSON samples without changing Filer-core production or normal development dependencies.
- [ ] Filer public-command, `std::fs::read_dir`, and `tokio::fs::read_dir` adapters cover matching fast and metadata flat-listing scenarios.
- [ ] Adapters emit canonical row counts and digests before their timing is accepted.
- [ ] Reports include first row, viewport, page, completion, work amplification, CPU, and peak-memory metrics when the adapter can observe them.
- [ ] Tests prove unsupported capabilities are explicit and that output or requested-field mismatches cannot enter a leaderboard.
