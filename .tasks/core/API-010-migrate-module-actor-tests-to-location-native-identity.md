---
id: API-010
title: Migrate module actor tests to Location-native identity
status: To Do
priority: High
type: TestDebt
parent: API-005
milestone: "0.3.0"
depends_on: [API-009]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports navigator, scanner, search, operator, and watcher tests off NodeId so actor coverage survives NodeId removal."
tags: [api, nodeid, location, testing]
last_updated: 2026-07-09
---

## Summary

Port the module actor tests that assert on NodeId or FileNode identity to LocationRef or NodeEntry identity. As of 2026-07-09 this cluster is the navigator_test modules (12 sites across 4 files), scanner_test modules (5 sites across 2 files), search_test (3), operator_test (5), and watcher_test (1) under filer-core/src/tests/modules. Depends on API-009 because these tests consume the shared fixtures it migrates.

## Acceptance Criteria

- [ ] No module actor test addresses nodes by NodeId or FileNode identity; each asserts on LocationRef or NodeEntry identity instead.
- [ ] The full filer-core suite passes with no reduction in test count.
