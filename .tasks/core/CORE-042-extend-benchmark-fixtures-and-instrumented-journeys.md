---
id: "CORE-042"
title: "Extend benchmark fixtures and instrumented journeys"
status: "Deferred"
priority: "Medium"
type: "Epic"
parent: "core:CORE-029"
depends_on: ["core:CORE-032"]
risk: "Medium"
tags: ["core", "benchmark", "performance"]
whitepaper: "docs/benchmarks/comparative-performance.md"
last_updated: "2026-09-05"
---

## Summary

Extend the initial browse journey with recursive search, sparse-match, Git, hostile-name, and mutation fixtures plus mutation-recovery and decorated-browse journeys. Add correlated provider/pipeline/queue attribution and concurrent responsiveness/cancellation measurements. Refine this map into fixture and journey stages before reactivation; the initial 0.3.1 result must remain independently useful.

## Rationale

The maintainer approved a smaller 0.3.1 benchmark slice on 2026-09-05. Extended scenarios follow the initial core comparison and browse journey, with no release milestone assigned yet.

## Exit Criteria

- [ ] The remaining fixture corpus has reproducible semantic expectations and platform capability records.
- [ ] Search, mutation-recovery, and decorated-browse journeys validate each visible milestone and record cancellation-to-quiescence and stale work.
- [ ] Correlated traces distinguish provider, pipeline, queue, view, and virtual-frame costs without changing production semantics.
