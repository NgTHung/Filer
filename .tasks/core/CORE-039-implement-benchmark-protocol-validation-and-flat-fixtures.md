---
id: "CORE-039"
title: "Implement benchmark protocol validation and flat fixtures"
status: "To Do"
priority: "High"
type: "Feature"
parent: "core:CORE-029"
milestone: "0.3.1"
depends_on: ["core:CORE-030"]
risk: "Medium"
tags: ["core", "benchmark", "performance"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Implement the CORE-030 protocol and deterministic flat-10k/flat-100k manifests inside the isolated filer-core/benchmarks package. Land schema validation and golden messages first, then fixture generation as a separate commit. Fixtures expose stable relative identities and requested metadata; provider-order results use order-independent membership validation unless a scenario explicitly requests ordering. Broader fixtures belong to CORE-042.

## Acceptance Criteria

- [ ] Protocol tests reject version mismatches, malformed events, missing required phases, duplicate rows, wrong digests, and unsupported scenarios reported as success.
- [ ] Both flat fixtures reproduce their expected membership and requested metadata from versioned manifests; generation stays outside timed samples.
- [ ] Production and normal development dependency graphs remain unchanged, and the isolated package tests pass.
