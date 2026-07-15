---
id: WEB-003
title: Serve ready-work and milestone aggregation endpoints
status: Done
priority: High
type: Feature
parent: WEB-001
risk: Low
impact: "Adds two read-only endpoints the v2 Ready and Milestones screens need; no write path changes."
tags: [web, tasks, api]
last_updated: 2026-07-15
---

## Summary

The v2 Ready screen needs the same 'To Do with every dependency finished' set the CLI's ready command already computes, and the Milestones screen needs each milestone-role task, its criteria, progress counts, and matching tasks grouped by status. Both are project-scoped reads over a freshly validated task list. Reuse filer-task's existing readiness and milestone-role logic, and add GET /api/projects/{project}/ready and GET /api/projects/{project}/milestones next to GET /api/projects/{project}/tasks.

## Acceptance Criteria

- [x] GET /api/projects/{project}/ready returns exactly the tasks filer-task's ready computation returns for the same project, honoring the same domain/milestone query filters as GET /api/projects/{project}/tasks.
- [x] GET /api/projects/{project}/milestones returns one entry per milestone-role task with its criteria, done/total counts, and matching tasks grouped by status.
- [x] Both endpoints reuse existing filer-task readiness/milestone logic rather than duplicating dependency or grouping computation in filer-task-web.
- [x] In-process tests cover empty results, ready ordering and filters, blocked/waiting task exclusion, milestone aggregation across several statuses, configured milestone roles, and project scoping.
