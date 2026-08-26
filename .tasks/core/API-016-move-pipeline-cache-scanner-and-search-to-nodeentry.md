---
id: "API-016"
title: "Move pipeline cache scanner and search to NodeEntry"
status: Done
priority: "High"
type: "Refactor"
parent: "API-007"
milestone: "0.3.0"
depends_on: ["API-015"]
rules: ["CORE-LIBRARY", "PROVIDER-ACCESS"]
risk: "High"
impact: "Removes the duplicate row and conversion hot path across directory transformation, caching, scanning, and search."
tags: ["api", "nodeid", "location"]
last_updated: 2026-08-26
---

## Summary

Run pipeline, query, paging selection, cache, scanner, and search directly on NodeEntry, then delete FileNode and all row conversion plumbing.

## Acceptance Criteria

- [x] NodeEntry carries LocationRef as its only identity; FileNode and every row conversion bridge are removed.
- [x] Pipeline, query, cache, scanner, search, and paging operate directly on NodeEntry with Location-keyed cache identity.
- [x] Behavioral, integration, full filer-core, and ignored stress tests pass with row assertions migrated rather than deleted.
