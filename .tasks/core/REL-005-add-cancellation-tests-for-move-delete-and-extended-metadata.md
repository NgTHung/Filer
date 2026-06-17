---
id: REL-005
title: Add cancellation tests for move, delete, and extended metadata
status: To Do
priority: Medium
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Low
impact: "Closes cancellation test gaps for already-cancellable long operations."
tags: [reliability, testing, cancellation]
last_updated: 2026-06-17
---

## Summary

Move, Delete, and Extended-Metadata loading already arm cancellation but lack tests. Add mid-flight cancellation tests reusing the MockProvider/MockPreviewProvider delay_ms + yield_now patterns. Out of scope: rename, create-file, create-folder, and metadata-load are non-cancellable by design, and provider-call timeout is owned by PROVIDER-001.

## Acceptance Criteria

- [ ] A test cancels a Move mid-flight and asserts it stops without a success event.
- [ ] A test cancels a Delete mid-flight and asserts it stops without a success event.
- [ ] A test cancels an Extended-Metadata load mid-flight and asserts no stale metadata event is emitted.
