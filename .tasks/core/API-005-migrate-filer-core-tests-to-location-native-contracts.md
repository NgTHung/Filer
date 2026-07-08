---
id: API-005
title: Migrate filer-core tests to Location-native contracts
status: To Do
priority: High
type: TestDebt
parent: API-004
milestone: "0.3.0"
depends_on: [CORE-025]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports NodeId-coupled tests to LocationRef identity so NodeId removal cannot delete coverage."
tags: [api, nodeid, location, testing]
last_updated: 2026-07-08
---

## Summary

Port the tests that assert on NodeId, FileNode rows, or compatibility command and event variants to Location-native contracts before any removal lands. 27 test files reference NodeId across roughly 105 sites. Where a test exercises a compatibility route slated for removal, replace it with the LocationRef-native equivalent that pins the same behavior; do not delete behavior coverage. This is the gate for API-006 through API-008: the 2026-07-08 removal attempt failed precisely because this step was skipped.

## Acceptance Criteria

- [ ] Every test that addresses nodes by NodeId or FileNode identity asserts on LocationRef or NodeEntry identity instead, except tests that exist solely to pin compatibility routes.
- [ ] Tests pinning compatibility-only routes are marked so API-006 can convert them to absence tests rather than delete them silently.
- [ ] The full filer-core suite passes with no reduction in test count beyond compatibility-only pins.
