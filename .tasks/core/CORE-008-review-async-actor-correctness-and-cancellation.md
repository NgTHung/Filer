---
id: CORE-008
title: Review async/actor correctness and cancellation
status: To Do
priority: High
type: TestDebt
parent: CORE-004
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: High
impact: "Cancellation and channel correctness gate the non-blocking large-directory proof target."
tags: [core, audit, async, cancellation]
last_updated: 2026-06-13
---

## Summary

Review per-session cancellation correctness, stale-result guards, task leaks, unbounded flume channel backpressure, and shutdown behavior across the actors.

## Acceptance Criteria

- [ ] Report at docs/reviews/filer-core/async-actors.md covers cancellation, stale-result guards, channel backpressure, and shutdown.
- [ ] Each potential task leak or stale-result race is documented with the actor and code path.
- [ ] Missing cancellation/backpressure test scenarios are listed as follow-up candidates.
