---
id: "CORE-029"
title: "Establish comparative core performance suite"
status: "To Do"
priority: "High"
type: "Epic"
risk: "Medium"
impact: "Adds peer and whole-pipeline evidence used to judge Filer-core performance direction."
tags: ["core", "performance", "benchmark", "testing"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Own the full comparative performance program while letting 0.3.1 finish after a useful first slice. CORE-030 defines the protocol, CORE-039 implements validation and flat fixtures, CORE-031 owns runner/adapters/report stages CORE-040 and CORE-041, and CORE-032 measures one browse journey. CORE-033, CORE-034, and CORE-042 retain later application, framework, recursive, and instrumented work outside 0.3.1. This milestone-free umbrella remains open until the full program is complete.

## Exit Criteria

- [ ] CORE-030, CORE-039, CORE-031, CORE-032, CORE-033, CORE-034, and CORE-042 are Done.
- [ ] Engine, core, reference-application, and external-application results remain separate with explicit timing boundaries.
- [ ] Every ranked sample passes scenario-specific semantic digest and row-count validation.
- [ ] Raw samples and generated reports cover flat browsing, recursive search, and whole-pipeline journeys.
- [ ] Benchmark-only dependencies stay outside Filer-core production and normal development dependency graphs.
