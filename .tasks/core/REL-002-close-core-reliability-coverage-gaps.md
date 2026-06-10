---
id: REL-002
title: Close core reliability coverage gaps
status: To Do
priority: High
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY, CORE-MECHANICS-BUILTIN]
risk: Medium
impact: "Validates cache freshness, event truthfulness, and cancellation under load."
tags: [reliability, testing, tracing]
last_updated: 2026-06-06
---

## Summary

Add regression and stress coverage for remaining app-facing reliability risks.

## Acceptance Criteria

- [ ] Manual refresh and same-folder navigation bypass stale cache entries.
- [ ] Tracing covers every app-facing command path.
- [ ] Rapid create, delete, and rename watcher bursts remain fresh and ordered.
- [ ] Long operations, search, preview, and provider calls have cancellation tests.
- [ ] Reproduced UI races become core contract regression tests when core owns the failure.
