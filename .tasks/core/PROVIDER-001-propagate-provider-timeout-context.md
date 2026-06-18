---
id: PROVIDER-001
title: Propagate provider timeout context
status: In Progress
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
depends_on: [REL-001]
rules: [PROVIDER-ACCESS, ACTOR-LONG-WORK]
risk: High
impact: "Touches provider calls, previews, search, operations, and cancellation behavior."
tags: [provider, timeout, cancellation]
last_updated: 2026-06-18
---

## Summary

Carry deadline and cancellation context through provider-backed long-running work.

## Acceptance Criteria

- [ ] Provider calls receive explicit timeout or deadline context.
- [ ] Preview, search, and operation actors propagate provider deadlines.
- [ ] Timeouts emit the stable TimedOut error code with provider context.
- [ ] Cancellation and timeout races have deterministic tests.
