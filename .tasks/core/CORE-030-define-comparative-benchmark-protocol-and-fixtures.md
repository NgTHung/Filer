---
id: "CORE-030"
title: "Define comparative benchmark protocol and fixtures"
status: "To Do"
priority: "High"
type: "Design"
parent: "CORE-029"
milestone: "0.3.1"
risk: "Low"
impact: "Fixes semantic equivalence, fixture identity, and result meaning before competitor measurements are trusted."
tags: ["core", "performance", "benchmark", "protocol", "testing"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Define the versioned adapter protocol, deterministic fixture corpus, scenario contracts, and sampling policy before competitor timings are accepted. Correctness, requested fields, cache state, and timing boundaries must be machine-readable so unlike work cannot enter the same ranking.

## Acceptance Criteria

- [ ] A versioned request and newline-delimited event schema carries scenario, fixture, implementation, cache, sample, phase, row-count, digest, status, and monotonic-time fields.
- [ ] Versioned manifests generate `flat-10k`, `flat-100k`, `tree-100k`, `sparse-match-100k`, `git-10k`, `hostile-10k`, and `mutation-10k` with stable expected digests.
- [ ] Browse, presentation, search, mutation, responsiveness, and whole-pipeline journey scenarios define start barriers, milestones, terminal conditions, and semantic output.
- [ ] Sampling rules separate cold start, cold filesystem, warm start, and warm steady state and record the machine, filesystem, build, version, and application configuration.
- [ ] Protocol and fixture conformance tests reject version mismatches, missing phases, wrong digests, duplicate rows, and unsupported scenarios reported as successful.
