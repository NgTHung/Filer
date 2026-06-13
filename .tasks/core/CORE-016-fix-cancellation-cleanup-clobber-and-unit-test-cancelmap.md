---
id: CORE-016
title: Fix cancellation cleanup clobber and unit-test CancelMap
status: To Do
priority: High
type: Bug
parent: CORE-004
milestone: "0.3.0"
rules: [ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: High
impact: "A stale task deletes the live cancel entry, so rapid re-issue and session destroy leave uncancellable orphaned work."
tags: [core, audit, remediation, cancellation]
last_updated: 2026-06-13
---

## Summary

Searcher and previewer clean up with the unconditional CancelMap::remove instead of remove_if_current. When a request is re-issued, the superseded task's cleanup deletes the new task's cancel token from the map, so a later arm or a session-destroy Cancel finds nothing to cancel and the live task runs orphaned. This is the one async correctness defect, and it bites search-as-you-type and preview-on-cursor-move, the common cases. The cancellation primitive itself (CancelMap) has zero direct unit tests, the crate's largest coverage gap. Switch the four searcher/previewer sites to remove_if_current, matching operator and scanner, and add direct CancelMap tests for the arm/stale-remove interleaving.

## Acceptance Criteria

- [ ] Searcher (searcher.rs:115) and previewer (previewer.rs:204, :292, :507, :584) use remove_if_current(session, &cancel) instead of remove.
- [ ] A direct CancelMap unit test arms a session twice and asserts a stale task's removal preserves the live token, failing against the old remove behavior.
- [ ] A rapid re-issue then cancel test for search and preview asserts the latest in-flight task is actually cancelled.
