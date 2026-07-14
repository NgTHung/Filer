---
id: WEB-001
title: Rebuild filer-task-web to the v2 design
status: To Do
priority: High
type: Epic
risk: Medium
impact: "Replaces the flat v1 task-web UI with the v2 design: ready-work, milestones, new-task creation, a filter-error-aware task list, a detail drawer with lifecycle actions, multi-project switching, project/policy configuration, task editing, and dark mode."
tags: [web, tasks, tooling]
last_updated: 2026-07-14
---

## Summary

Implements the imported Filer Tasks v2 design (claude.ai/design project 3ae75c47-3998-44b3-89e4-c455fa75f5fd), extended past what the mockup itself designed. v1 (core:UTILS-004) shipped a minimal list+filter+detail UI over a single project. Now that core:UTILS-005's portability work is done, this epic adds the backend capability (multi-project registry, ready/milestones reads, task creation, criteria toggling) and rebuilds the frontend to match the v2 mockup: sidebar nav with domain counts, a Ready-work screen, a Tasks screen with a filter-chip menu (including ambiguous_reference and unknown_tag error states), a Milestones screen, a New-task form, a task detail drawer with block/defer/obsolete reason forms and a Done refusal when acceptance criteria are unchecked, and a command-K project-switcher palette. The mockup itself has no way to find or create a project (only switch between already-registered ones), no way to configure a project's domains/prefixes/types/tags, no way to edit a task's own fields after creation, and only one hardcoded light theme; this epic closes those four gaps too, on top of new core:UTILS-013 library primitives (project init, policy mutation, task field edit, criteria toggle) that v1's and the mockup's write surface never needed.

## Exit Criteria

- [ ] GET /api/ready lists To Do tasks with no unfinished dependency, and GET /api/milestones aggregates status counts per milestone-role task.
- [ ] POST /api/tasks creates a task file through the existing filer-task create path, and a criteria-toggle endpoint flips one checklist item without other writes.
- [ ] The web app discovers and lists more than one .tasks project, can switch between them, and surfaces a project that fails validation without disabling the others.
- [ ] The web app can find or create a project by path and edit its domain/prefix/type/tag policy, and an already-created task's own fields can be edited after the fact.
- [ ] The static frontend matches the v2 design's five screens (Ready, Tasks, Milestones, New task, and the task detail drawer) and the command palette, wired to real endpoints instead of mock data, plus a Settings screen, in-drawer task editing, and a light/dark theme toggle the mockup didn't design.
- [ ] In-process API tests cover the new endpoints and filer-task validate passes after web-driven writes.
