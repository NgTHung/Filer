---
id: REL-003
title: Add cache freshness regression tests
status: Done
priority: High
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY, CORE-MECHANICS-BUILTIN]
risk: Medium
impact: "Proves stale directory listings cannot be served after a change or manual refresh."
tags: [reliability, testing, cache]
last_updated: 2026-06-21
---

## Summary

Add regression tests proving manual refresh bypasses stale DirCache entries and same-folder navigation serves cache. Manual refresh routes Command::Refresh to ScanCommand::RefreshLocation/RefreshNode with invalidate_cache=true; same-folder navigation uses ScanNode with invalidate_cache=false. Pure test, no production change.

## Acceptance Criteria

- [x] A test caches a directory, mutates the filesystem, triggers Refresh, and asserts the fresh listing is returned (stale entry bypassed).
- [x] A test asserts same-folder navigation serves the cached listing without re-listing the provider.
