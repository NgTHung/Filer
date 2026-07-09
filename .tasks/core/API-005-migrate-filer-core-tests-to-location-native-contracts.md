---
id: API-005
title: Migrate filer-core tests to Location-native contracts
status: To Do
priority: High
type: Epic
parent: API-004
milestone: "0.3.0"
depends_on: [CORE-025]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports NodeId-coupled tests to LocationRef identity so NodeId removal cannot delete coverage."
tags: [api, nodeid, location, testing]
last_updated: 2026-07-09
---

## Summary

Port the tests that assert on NodeId, FileNode rows, or compatibility command and event variants to Location-native contracts before any removal lands. As of 2026-07-09, 25 test files reference NodeId across 91 sites, too much for one reviewable change under the size guidance, so this epic stages the migration by test cluster: model and fixture tests (API-009), module actor tests (API-010), command router and API tests (API-011), and top-level integration tests (API-012). API-009 lands first because the other clusters consume its fixtures.

Where a test exercises a compatibility route slated for removal, replace it with the LocationRef-native equivalent that pins the same behavior; do not delete behavior coverage. This epic is the gate for API-006 through API-008: the 2026-07-08 removal attempt failed precisely because this step was skipped.

## Exit Criteria

- [ ] API-009, API-010, API-011, and API-012 are Done.
- [ ] No filer-core test addresses nodes by NodeId or FileNode identity, except tests that exist solely to pin compatibility routes.
- [ ] Tests pinning compatibility-only routes are marked so API-006 can convert them to absence tests rather than delete them silently.
- [ ] The full filer-core suite passes with no reduction in test count beyond compatibility-only pins.
