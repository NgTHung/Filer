---
id: CORE-022
title: Consolidate test fixtures and stabilize async test synchronization
status: To Do
priority: Medium
type: TestDebt
parent: CORE-001
risk: Low
impact: "Conflicting fixtures and timing-based synchronization make the suite hard to read and flaky under load."
tags: [core, audit, remediation, testing]
last_updated: 2026-07-08
---

## Summary

The test tree duplicates its fixtures. Counts refreshed 2026-07-08 after the CORE-026 split: make_file is defined eight times across five files (query_test, operator_test, scanner_test, search_test, pipeline fixtures) with a tail of make_file_at/make_hidden variants; MockProvider is defined twice, plus per-suite MockFs, MockOpsProvider, and MockPreviewProvider. A change to the provider trait forces edits in every copy. Extract a tests/support module with one node builder and one configurable mock. Separately, async actor tests synchronize with fixed sleep plus timeout races (watcher_test 20 sites, navigator_test 12), which are load-sensitive and inflate wall-clock; convert the watcher and navigator suites to event-driven synchronization that awaits the actual event or channel with a single generous outer deadline. Coordinate with API-005: if the fixture consolidation lands first, the test migration builds on the shared support module instead of touching every copy.

## Acceptance Criteria

- [ ] A tests/support module exports one node builder and one configurable mock provider, replacing the duplicated make_file signatures and mock provider copies.
- [ ] The watcher and navigator suites synchronize on the actual event or channel with a single outer deadline rather than fixed inner sleeps.
- [ ] The full filer-core test suite passes after the consolidation with no loss of coverage.
