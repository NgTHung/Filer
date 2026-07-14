---
id: UTILS-016
title: Edit an existing task's stored fields
status: Done
priority: High
type: Feature
parent: UTILS-013
risk: Medium
impact: "Adds the only write path that changes a task's own metadata or body text after creation; every other mutator only changes status or appends child tasks."
tags: [tasks, library, validation]
last_updated: 2026-07-14
---

## Summary

lifecycle.rs only has add_task (create) and the five status transitions; nothing rewrites title, summary, body sections, risk, impact, tags, milestone, parent, or depends_on on a task that already exists. Add an edit_task(project, id, patch) function that applies a partial patch, re-runs the same validation path task creation and the lifecycle transitions already use, and writes atomically. Changing parent or depends_on must still be checked for cycles; changing milestone must still resolve to a real milestone-role task.

## Acceptance Criteria

- [x] edit_task accepts a partial patch (only supplied fields change) and re-validates the whole task, including relationship fields, before writing.
- [x] Editing parent or depends_on to a value that would introduce a cycle is rejected with the same cycle error validate_repo reports, and the file is left unchanged.
- [x] Editing milestone to a value with no matching milestone-role task is rejected before any write happens.
- [x] The write is atomic, matching the guarantee core:UTILS-004 established for task file writes.
- [x] Tests cover a successful multi-field edit, a rejected cyclic parent/depends_on edit, a rejected unknown milestone edit, and byte-for-byte no-op when the patch is empty.
