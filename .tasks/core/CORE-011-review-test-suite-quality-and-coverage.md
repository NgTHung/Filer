---
id: CORE-011
title: Review test-suite quality and coverage
status: To Do
priority: Medium
type: TestDebt
parent: CORE-004
risk: Medium
impact: "Coverage gaps in a TDD project let regressions reach the public contract unseen."
tags: [core, audit, testing]
last_updated: 2026-06-13
---

## Summary

Review the test-to-code ratio, coverage gaps, fixture patterns, and missing cancellation/timeout tests. Note overlap with REL-002 rather than duplicating it.

## Acceptance Criteria

- [ ] Report at docs/reviews/filer-core/test-suite.md inventories coverage by subsystem and flags gaps.
- [ ] Fixture and test-style inconsistencies are documented with examples.
- [ ] Follow-up test-debt candidates are listed and cross-referenced with REL-002.
