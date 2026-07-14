---
id: WEB-004
title: Add task creation and criteria-toggle write endpoints
status: To Do
priority: High
type: Feature
parent: WEB-001
depends_on: [core:UTILS-017]
risk: Medium
impact: "Adds the two remaining write paths the v2 UI needs (creating a task file, checking a single criterion) on top of the existing write-lock and revalidate pattern used by the lifecycle transitions."
tags: [web, tasks, api]
last_updated: 2026-07-14
---

## Summary

The v2 New-task form posts domain, prefix, number, title, type, priority, milestone, and tags and expects field-scoped errors (id_exists, prefix_not_allowed, tag_rejected) it can render inline, never a modal. The drawer's acceptance-criteria list toggles one item at a time. Add POST /api/tasks that calls into filer-task's existing task-creation path (do not hand-build frontmatter/markdown in filer-task-web) and POST /api/tasks/{id}/criteria/{index} that calls core:UTILS-017's toggle_criterion, both under the existing write_lock and both returning the refreshed ShowView like the lifecycle transitions do. core:UTILS-017 provides the checklist-toggle primitive; this task is the thin web-layer wrapper over it, not a reimplementation.

## Acceptance Criteria

- [ ] POST /api/tasks creates a task file through filer-task's existing creation path and returns the new task's ShowView.
- [ ] A rejected creation (duplicate id, disallowed prefix, disallowed tag under strict policy) returns a structured error with the offending field name and the original filer-task reason code.
- [ ] POST /api/tasks/{id}/criteria/{index} toggles exactly one checklist item and leaves the rest of the file unchanged.
- [ ] Both endpoints serialize with the existing write_lock and re-validate before responding, matching the pattern in routes/transitions.rs.
- [ ] In-process tests cover a successful creation, each rejection reason code, and a criteria toggle round-trip.
