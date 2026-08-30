---
id: API-008
title: Delete the NodeId type and prove absence
status: Done
priority: High
type: Refactor
parent: API-004
milestone: "0.3.0"
depends_on: [API-007]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Removes the last NodeId definitions and pins the removal so it cannot regress."
tags: [api, nodeid, location]
last_updated: 2026-08-30
---

## Summary

Delete the NodeId type, constructors, hashing helpers, and remaining exports once nothing references them. Pin the removal with an absence check so a future change cannot quietly reintroduce transient identity.

## Acceptance Criteria

- [x] No NodeId type, constructor, hashing helper, or export remains in filer-core source or tests.
- [x] An absence test or snapshot pins that NodeId identifiers do not reappear in the public API.
- [x] The full filer-core suite passes.
