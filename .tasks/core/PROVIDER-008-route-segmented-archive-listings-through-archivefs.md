---
id: PROVIDER-008
title: Route segmented archive listings through ArchiveFs
status: Done
priority: Medium
type: Refactor
parent: PROVIDER-002
depends_on: [PROVIDER-007]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: Medium
impact: "Decouples segmented location routing from archive parsing internals."
tags: [provider, vfs, archive]
last_updated: 2026-06-24
---

## Summary

Move ZIP traversal out of SegmentedLocationResolver so segmented local archive locations list through ArchiveFs.

## Acceptance Criteria

- [x] SegmentedLocationResolver no longer owns ZIP directory parsing or ZIP entry naming helpers.
- [x] Segmented local ZIP locations still produce Location-native NodeEntry values.
- [x] Virtual segments and unsupported provider routes keep structured errors.
