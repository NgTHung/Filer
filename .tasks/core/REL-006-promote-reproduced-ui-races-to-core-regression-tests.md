---
id: REL-006
title: Promote reproduced UI races to core regression tests
status: To Do
priority: Low
type: TestDebt
parent: CORE-001
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Low
impact: "Keeps reproduced UI race failures from regressing once core owns the fix."
tags: [reliability, testing]
last_updated: 2026-07-09
---

## Summary

Standing task: when a UI race is reproduced and core owns the failure, capture it as a core contract regression test. Activated only when such a race is reproduced; no speculative work.

Not a 0.3.0 exit gate. Milestone membership was removed so an open-ended standing task cannot block MILESTONE-003.

## Acceptance Criteria

- [ ] Each reproduced UI race that core owns has a corresponding core regression test.
