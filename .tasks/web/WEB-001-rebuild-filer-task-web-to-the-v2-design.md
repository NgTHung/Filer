---
id: WEB-001
title: Rebuild filer-task-web to the v2 design
status: To Do
priority: High
type: Epic
risk: Medium
impact: "Replaces the flat v1 task-web UI: ready-work, milestones, new-task creation, a policy-aware task list, a detail drawer with lifecycle actions, multi-project switching, project/policy configuration, task editing, and a light/dark theme."
tags: [web, tasks, tooling]
last_updated: 2026-07-15
---

## Summary

Rebuilds filer-task-web around the screens the Filer Tasks v2 design proposed (claude.ai/design project 3ae75c47-3998-44b3-89e4-c455fa75f5fd, kept as provenance; the task-level specs are grounded in the implemented API, not the mockup). The backend half is largely landed: a multi-project registry with project-scoped routes (web:WEB-002), ready and milestone reads (web:WEB-003), and task creation plus criteria set via PUT with an If-Match content hash (web:WEB-004). Remaining backend work adds project creation and policy mutation (web:WEB-009), task field edits (web:WEB-010), and a policy read endpoint with strict filter validation (web:WEB-021). The frontend is rebuilt on a vendored Preact + htm shell (web:WEB-005): sidebar nav with domain counts, a Ready-work screen, a Tasks screen with a policy-aware filter-chip menu, a Milestones screen, a New-task form, a task detail drawer with block/defer/obsolete reason forms and a Done refusal when acceptance criteria are unchecked, a command-K project-switcher palette, a Settings screen for project and policy management, in-drawer field editing, and a light/dark theme toggle.

## Exit Criteria

- [ ] GET /api/projects/{project}/ready lists To Do tasks with no unfinished dependency, and GET /api/projects/{project}/milestones aggregates status counts per milestone-role task.
- [ ] POST /api/projects/{project}/tasks creates a task file through the existing filer-task create path, and PUT /api/projects/{project}/tasks/{id}/criteria/{index} sets one checklist item, guarded by an If-Match content hash, without other writes.
- [ ] The web app discovers and lists more than one .tasks project, can switch between them, and surfaces a project that fails validation without disabling the others.
- [ ] The web app can find or create a project by path and edit its domain/prefix/type/tag policy, and an already-created task's own fields can be edited after the fact.
- [ ] The static frontend ships the five core screens (Ready, Tasks, Milestones, New task, and the task detail drawer) and the command palette, wired to the project-scoped endpoints, plus a Settings screen, in-drawer task editing, and a light/dark theme toggle.
- [ ] In-process API tests cover the new endpoints and filer-task validate passes after web-driven writes.
