---
id: WEB-012
title: Add task field editing to the detail drawer
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-008, WEB-010]
risk: Low
impact: "Adds edit-in-place to the read-plus-lifecycle drawer, closing the last human-in-the-loop gap: changing a task's own fields after creation."
tags: [web, tasks]
last_updated: 2026-07-15
---

## Summary

The drawer (web:WEB-008) renders title, summary, type/milestone/risk/impact/tags, and relationships as read-only text and chips, with edits limited to lifecycle actions and criteria toggles. Add an Edit affordance per section (or a single Edit mode toggle for the drawer body) that turns title, summary, risk, impact, tags, milestone, parent, and depends_on into inputs, and posts a partial patch to PATCH /api/projects/{project}/tasks/{id} (web:WEB-010) on save, rendering the structured error body's field-scoped errors inline exactly like the New-task form (web:WEB-007) does.

## Acceptance Criteria

- [ ] An edit affordance in the drawer turns title, summary, risk, impact, tags, milestone, parent, and depends_on into editable inputs pre-filled with current values.
- [ ] Saving posts only the changed fields to PATCH /api/projects/{project}/tasks/{id} and the drawer refreshes from the response.
- [ ] A rejected edit renders its field-scoped error inline on the offending input, matching the New-task form's error rendering pattern.
- [ ] Canceling an edit discards changes and restores the read-only view without a network call.
