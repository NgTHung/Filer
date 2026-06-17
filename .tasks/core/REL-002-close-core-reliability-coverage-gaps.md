---
id: REL-002
title: Add command-path tracing coverage
status: Done
priority: High
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY, CORE-MECHANICS-BUILTIN]
risk: Medium
impact: "Makes every app-facing command observable for debugging and reliability triage."
tags: [reliability, testing, tracing]
last_updated: 2026-06-17
---

## Summary

Instrument the single command dispatch choke point (`CommandRouter::route`) so every app-facing command path emits one structured trace record carrying key, session, request, and operation. Add a tracing-capture test proving the records appear. This is the first stage split out of the original reliability-coverage umbrella; cache freshness, watcher bursts, and cancellation gaps move to REL-003, REL-004, and REL-005.

## Acceptance Criteria

- [x] Every command dispatched through `CommandRouter::route` emits one structured trace record carrying key, session, request, and operation.
- [x] The record is emitted only after session validation, so rejected unknown-session commands are not counted.
- [x] A test installs a capturing subscriber and asserts a record with the correct key and session for one command per drivable family (navigate, scan, search, preview, ops, session lifecycle).
