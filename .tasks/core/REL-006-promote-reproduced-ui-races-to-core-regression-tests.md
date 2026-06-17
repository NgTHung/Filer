---
id: REL-006
title: Promote reproduced UI races to core regression tests
status: To Do
priority: Low
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Low
impact: "Keeps reproduced UI race failures from regressing once core owns the fix."
tags: [reliability, testing]
last_updated: 2026-06-17
---

## Summary

Standing task: when a UI race is reproduced and core owns the failure, capture it as a core contract regression test. Activated only when such a race is reproduced; no speculative work.

## Acceptance Criteria

- [ ] Each reproduced UI race that core owns has a corresponding core regression test.
