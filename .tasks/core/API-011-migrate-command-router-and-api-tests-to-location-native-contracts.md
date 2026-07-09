---
id: API-011
title: Migrate command router and API tests to Location-native contracts
status: To Do
priority: High
type: TestDebt
parent: API-005
milestone: "0.3.0"
depends_on: [API-009]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports command routing tests to LocationRef contracts and marks compatibility-only pins so API-006 can remove routes without silent coverage loss."
tags: [api, nodeid, location, testing]
last_updated: 2026-07-09
---

## Summary

Port the command router and API tests that exercise NodeId routes or compatibility command and event variants to Location-native contracts. As of 2026-07-09 this cluster is command_api_test.rs (3 sites), command_router_test/harness.rs (2), route_navigation.rs (7), route_unresolved_node_boundaries.rs (6), route_unwatch_session_to_watcher.rs (4), and route_load_preview_with_options.rs (2) under filer-core/src/tests/api. Where a test exercises a compatibility route slated for removal, replace it with the LocationRef-native equivalent pinning the same behavior; do not delete behavior coverage. Depends on API-009 for Location-native fixtures.

## Acceptance Criteria

- [ ] Every router or API test that addresses nodes by NodeId asserts on LocationRef or NodeEntry identity instead, except tests that exist solely to pin compatibility routes.
- [ ] Tests pinning compatibility-only routes are marked so API-006 can convert them to absence tests rather than delete them silently.
- [ ] The full filer-core suite passes with no reduction in test count beyond compatibility-only pins.
