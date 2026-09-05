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
last_updated: 2026-09-05
---

## Summary

API-005 is Done. tests/support/mod.rs already supplies a NodeEntry builder and event waiters. Top-level scanner, search, and navigation integration suites still duplicate node builders and provider doubles; internal suites have additional local fixtures. CORE-037 consolidates reusable setup by test cluster while preserving doubles that model distinct provider behavior. CORE-038 separately replaces watcher and navigator timing-based synchronization with observable event barriers.

Migrate one test cluster per commit. Reuse the existing support module and name remaining intentional specialized doubles in the completion evidence. Sharing a configurable fixture must not hide which provider behavior a test exercises.

## Acceptance Criteria

- [ ] CORE-037 is Done: shared NodeEntry construction and reusable provider setup replace duplicated fixtures, with intentional specialized doubles documented.
- [ ] CORE-038 is Done: watcher and navigator suites use event or channel barriers with one outer deadline; sleeps remain only where elapsed time is the behavior under test.
- [ ] The full filer-core test suite passes after the consolidation with no loss of coverage.
