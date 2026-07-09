---
id: CORE-022
title: Consolidate test fixtures and stabilize async test synchronization
status: To Do
priority: Medium
type: TestDebt
parent: CORE-027
milestone: "0.3.1"
risk: Low
impact: "Conflicting fixtures and timing-based synchronization make the suite hard to read and flaky under load."
tags: [core, audit, remediation, testing]
last_updated: 2026-07-09
---

## Summary

The test tree duplicates its fixtures. Counts refreshed 2026-07-09 after the CORE-026 split: make_file (and close variants) is defined about eleven times across the suite with a tail of make_file_at/make_hidden helpers; MockProvider and per-suite mock variants remain duplicated. A change to the provider trait forces edits in every copy. Extract a tests/support module with one node builder and one configurable mock. Separately, async actor tests synchronize with fixed sleep plus timeout races (watcher_test about 40 timing sites, navigator_test about 30 across split files), which are load-sensitive and inflate wall-clock; convert the watcher and navigator suites to event-driven synchronization that awaits the actual event or channel with a single generous outer deadline. Coordinate with API-005: if the fixture consolidation lands first, the test migration builds on the shared support module instead of touching every copy.

Milestone 0.3.1 (MILESTONE-004 draft). Tracked under CORE-027 post-audit remediation.

## Acceptance Criteria

- [ ] A tests/support module exports one node builder and one configurable mock provider, replacing the duplicated make_file signatures and mock provider copies.
- [ ] The watcher and navigator suites synchronize on the actual event or channel with a single outer deadline rather than fixed inner sleeps.
- [ ] The full filer-core test suite passes after the consolidation with no loss of coverage.
