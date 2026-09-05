---
id: "CORE-040"
title: "Add isolated benchmark runner and Filer adapter"
status: "To Do"
priority: "High"
type: "Feature"
parent: "core:CORE-031"
milestone: "0.3.1"
depends_on: ["core:CORE-039"]
risk: "Medium"
tags: ["core", "benchmark", "performance"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Implement one runner for the validated protocol and a public-command Filer adapter. Cover fast and metadata first page, continuation, and full completion using the flat fixtures. Preserve request/session correlation and store raw JSON with machine, build, fixture, and cache state. Land runner lifecycle before the Filer adapter and keep each complex diff below 700 changed lines.

## Acceptance Criteria

- [ ] The runner validates output before accepting timings and records failures with raw diagnostics.
- [ ] The Filer adapter reports page, completion, emitted-row counts, and observable work metrics without private core hooks or fabricated row-arrival timing.
- [ ] Tests cover wrong counts/digests, duplicate events, cancellation cleanup, and unsupported metrics; the isolated benchmark tests pass.
