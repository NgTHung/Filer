---
id: "CORE-029"
title: "Establish comparative core performance suite"
status: "To Do"
priority: "High"
type: "Epic"
milestone: "0.3.1"
risk: "Medium"
impact: "Adds peer and whole-pipeline evidence used to judge Filer-core performance direction."
tags: ["core", "performance", "benchmark", "testing"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-08-26"
---

## Summary

Build a versioned comparative benchmark suite around Filer-core without adding benchmark dependencies to production code. Keep engine, public-core, reference-application, and external-application results separate so each comparison has an honest timing boundary.

The suite builds on the internal regression evidence from CORE-028 but does not wait for its paging and decoration gates. Competitor evidence should help choose those implementations, not arrive after every optimization is complete.

## Exit Criteria

- [ ] CORE-030, CORE-031, CORE-032, CORE-033, and CORE-034 are Done.
- [ ] Engine, core, reference-application, and external-application results use separate leaderboards with explicit timing boundaries.
- [ ] Every ranked sample passes semantic digest and row-count validation before its timing is accepted.
- [ ] Raw samples and generated reports cover flat browsing, recursive search, and at least one whole-pipeline journey.
- [ ] Benchmark-only and system-framework dependencies stay outside Filer-core production and normal development dependency graphs.
