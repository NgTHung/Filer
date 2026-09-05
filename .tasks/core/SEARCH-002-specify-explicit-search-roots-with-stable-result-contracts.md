---
id: "SEARCH-002"
title: "Specify explicit search roots with stable result contracts"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:SEARCH-001"
milestone: "0.5.0"
depends_on: ["milestones:MILESTONE-005"]
rules: ["PROVIDER-ACCESS", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["core"]
last_updated: "2026-09-05"
---

## Summary

Define the first SEARCH-001 slice: a request searches an explicit set of provider-aware roots while preserving existing result and cancellation contracts. The client resolves workspace membership into roots. Inspect SearchQuery, provider routing, and Searcher before specifying overlapping-root behavior. Indexing and extension contributions stay later stages.

## Acceptance Criteria

- [ ] The contract defines empty, duplicate, overlapping, unauthorized, and unavailable roots and reports partial failure explicitly.
- [ ] Tests are specified for request/session isolation, cancellation, stale results, and duplicate matches across roots.
- [ ] Bounded implementation children cover root routing and regression coverage; native delegation, indexing, and extension work remain separate mapped stages.
