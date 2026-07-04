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
last_updated: 2026-07-04
---

## Summary

The test tree is the crate's largest maintainability violator. make_file exists in four mutually incompatible signatures across files, with a long tail of make_file_at/make_hidden variants; MockProvider is defined five times, MockFs twice, plus per-suite MockOpsProvider and MockPreviewProvider; build_core is reimplemented with the same name and different parameter types. A change to the provider trait forces edits in seven places. Extract a tests/support module with one FileNodeBuilder and one configurable mock, and a single build_core harness. Separately, async actor tests synchronize with fixed sleep plus timeout races (watcher_test 39 sites, navigator_test 30), which are load-sensitive and inflate wall-clock; convert the watcher and navigator suites to event-driven synchronization that awaits the actual event or channel with a single generous outer deadline.

## Acceptance Criteria

- [ ] A tests/support module exports one FileNodeBuilder and one configurable mock provider, replacing the four make_file signatures, the five MockProvider copies, and the duplicated build_core harnesses.
- [ ] The watcher and navigator suites synchronize on the actual event or channel with a single outer deadline rather than fixed inner sleeps.
- [ ] The full filer-core test suite passes after the consolidation with no loss of coverage.
