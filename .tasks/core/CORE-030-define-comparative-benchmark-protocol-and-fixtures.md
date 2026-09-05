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
last_updated: "2026-09-05"
---

## Summary

Specify the initial adapter protocol, flat fixture manifests, and browse scenario contracts in docs/benchmarks/comparative-performance.md. This is a design deliverable with normative valid/invalid JSON examples and a conformance matrix. CORE-039 owns executable validation and fixture generation. Define extension points for later scenarios without requiring the full fixture corpus or adapters to close this task.

## Acceptance Criteria

- [ ] A versioned schema and valid/invalid examples define requests, events, identities, required phases, counts, digests, statuses, and one monotonic clock domain.
- [ ] flat-10k and flat-100k manifests define reproducible relative identities, requested metadata, and canonical digests; provider enumeration order is not assumed stable.
- [ ] Fast/metadata browse, continuation, name sort, filter, and refresh specify barriers, visible milestones, and completion for the initial reference journey.
- [ ] Sampling and unavailable-metric rules identify cache state, machine, filesystem, build, and adapter version; correctness gates distinguish streaming, sparse filtering, and snapshot-only transforms.
- [ ] A conformance matrix maps every rejection and scenario gate to tests CORE-039 will implement; extended corpus and journeys are assigned to CORE-042.
