---
id: "API-015"
title: "Make providers and paging emit NodeEntry"
status: "To Do"
priority: "High"
type: "Refactor"
parent: "API-007"
milestone: "0.3.0"
depends_on: ["API-014"]
rules: ["CORE-LIBRARY", "PROVIDER-ACCESS"]
risk: "High"
impact: "Moves provider and paging output to reconstructable Location-native rows before the pipeline cutover."
tags: ["api", "nodeid", "location"]
last_updated: "2026-08-14"
---

## Summary

Change provider listing, metadata, and paging contracts to emit NodeEntry with full local or archive-member Location identity. Keep any temporary pipeline bridge private and centralized for removal by API-016.

## Acceptance Criteria

- [ ] FsProvider listing and metadata methods plus DirectoryPageResult emit NodeEntry.
- [ ] Local, archive, segmented, mock, and integration providers preserve reconstructable Location identity and existing listing behavior.
- [ ] Fast and metadata listings, native and fallback paging, cancellation, provider tests, and the full filer-core suite pass without a new public compatibility row.
