---
id: "CORE-038"
title: "Replace watcher and navigator timing guesses with event barriers"
status: "To Do"
priority: "Medium"
type: "TestDebt"
parent: "core:CORE-022"
milestone: "0.3.1"
rules: ["ACTOR-LONG-WORK", "SESSION-BOUNDARY"]
risk: "Low"
tags: ["core", "testing", "async"]
last_updated: "2026-09-05"
---

## Summary

Audit watcher and navigator tests, including split src/tests/modules suites and navigation_flow_test, for sleeps used to guess actor readiness or delivery. Convert one suite per commit to event/channel barriers with an outer deadline and useful failure context. Reuse existing event waiters where they fit. Elapsed-time behavior such as debounce may retain timed assertions with a recorded reason. This work can land independently of fixture consolidation.

## Acceptance Criteria

- [ ] Watcher and navigator setup, transitions, and teardown await the event or channel state they assert instead of sleeping for an assumed scheduling delay.
- [ ] Remaining sleeps are identified in completion evidence as tests of elapsed-time behavior, and timeouts report the missing event.
- [ ] Focused suites pass repeatedly while tests run concurrently; the full filer-core suite passes without removing behavior coverage.
