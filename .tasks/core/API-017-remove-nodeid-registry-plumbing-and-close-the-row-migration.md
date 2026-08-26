---
id: "API-017"
title: "Remove NodeId registry plumbing and close the row migration"
status: In Progress
priority: "High"
type: "Refactor"
parent: "API-007"
milestone: "0.3.0"
depends_on: ["API-016"]
rules: ["CORE-LIBRARY", "PROVIDER-ACCESS"]
risk: "Medium"
impact: "Leaves Location registration as the only registry authority and makes the final NodeId deletion isolated to API-008."
tags: ["api", "nodeid", "location"]
last_updated: 2026-08-26
---

## Summary

Remove the remaining NodeId registry maps and compatibility helpers, update current documentation, and prove API-007 left only the NodeId definition and API-008 deletion pin.

## Acceptance Criteria

- [ ] NodeRegistry retains only Location registration, descriptor lookup, route caching, LocationRef resolution, and local path to Location construction.
- [ ] No production field, enum variant, registry, cache, provider, pipeline, scanner, searcher, navigator, watcher, or operation references NodeId or FileNode.
- [ ] Documentation, absence checks, the full filer-core suite, and ignored stress tests pass while the NodeId definition and API-008 deletion pin remain.
