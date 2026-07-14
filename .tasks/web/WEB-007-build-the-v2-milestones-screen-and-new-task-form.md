---
id: WEB-007
title: Build the v2 Milestones screen and New-task form
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-003, WEB-004]
risk: Low
impact: "Adds the two screens v1 never had: a milestone progress view and an in-app task-creation form, closing the human-in-the-loop gap UTILS-004 left open for creation."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

Milestones screen: one card per GET /api/milestones entry with its exit/acceptance criteria, a progress bar from the done/total count, and its tasks grouped by status. New-task screen: domain, then prefix (scoped to the chosen domain's allowed prefixes), an auto-suggested next number, title, type, priority, optional milestone, and tags (chip picker under strict policy, free-text input under open policy), with a live preview of the qualified id and file path, submitting to POST /api/tasks and rendering field-scoped errors from its response inline.

## Acceptance Criteria

- [ ] The Milestones screen renders one card per milestone-role task with its progress bar and criteria checklist sourced from GET /api/milestones.
- [ ] The New-task form's prefix options update when the domain changes, and the number field defaults to one past the highest existing number for that domain/prefix.
- [ ] A field-scoped error returned by POST /api/tasks (id_exists, prefix_not_allowed, tag_rejected) renders next to its field, never as a modal or toast.
- [ ] A successful submission navigates to the Tasks screen with the new task's detail drawer open.
