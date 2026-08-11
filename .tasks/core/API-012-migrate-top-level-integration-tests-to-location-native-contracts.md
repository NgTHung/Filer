---
id: API-012
title: Migrate top-level integration tests to Location-native contracts
status: Done
priority: High
type: TestDebt
parent: API-005
milestone: "0.3.0"
depends_on: [API-009]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports end-to-end integration tests off NodeId so full-flow coverage survives NodeId removal."
tags: [api, nodeid, location, testing]
last_updated: 2026-08-11
---

## Summary

Port the top-level integration tests under filer-core/tests that assert on NodeId or exercise compatibility variants to Location-native contracts. As of 2026-07-09 this cluster is navigation_flow_test.rs (3 sites), command_tracing_test.rs (4), stress_test.rs (3), search_integration_test.rs (3), and scanner_integration_test.rs (2). Where a test exercises a compatibility route slated for removal, replace it with the LocationRef-native equivalent pinning the same behavior. Depends on API-009 for Location-native fixtures.

## Acceptance Criteria

- [x] No integration test addresses nodes by NodeId or FileNode identity, except tests that exist solely to pin compatibility routes.
- [x] Any compatibility-only pins are marked so API-006 can convert them to absence tests rather than delete them silently.
- [x] The full filer-core suite passes with no reduction in test count beyond compatibility-only pins.
