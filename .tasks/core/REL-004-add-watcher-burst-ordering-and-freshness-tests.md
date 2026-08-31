---
id: REL-004
title: Add watcher burst ordering and freshness tests
status: In Progress
priority: Medium
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Medium
impact: "Validates that rapid filesystem change bursts stay ordered and keep the cache fresh."
tags: [reliability, testing, watcher]
last_updated: 2026-08-31
---

## Summary

Add a deterministic burst test through the existing TestWatchProvider. Register three distinct local LocationRef watches, inject one synthetic create, delete, and rename change for those locations without filesystem timing, and assert the ordered FsChanged events and refresh invalidations. This is pure test coverage; no production change is expected.

## Acceptance Criteria

- [ ] A test injects a mixed create/delete/rename burst for distinct watched locations and asserts the LocationRef-scoped FsChanged events are emitted in the order injected.
- [ ] The burst test asserts each watched location produces exactly one NavCommand::Invalidate refresh signal, with no duplicate signals.
