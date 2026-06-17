---
id: REL-004
title: Add watcher burst ordering and freshness tests
status: To Do
priority: Medium
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Medium
impact: "Validates that rapid filesystem change bursts stay ordered and keep the cache fresh."
tags: [reliability, testing, watcher]
last_updated: 2026-06-17
---

## Summary

Add deterministic burst tests via the existing TestWatchProvider: multi-file create, delete, and rename bursts asserting event ordering and post-burst cache invalidation. Avoid real-filesystem timing by injecting synthetic FsChange events. Pure test, no production change.

## Acceptance Criteria

- [ ] A test injects a mixed create/delete/rename burst and asserts events are emitted in the order injected.
- [ ] A test asserts each watched node in a burst triggers cache invalidation exactly once per node.
