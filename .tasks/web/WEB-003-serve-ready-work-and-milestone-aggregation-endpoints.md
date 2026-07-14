---
id: WEB-003
title: Serve ready-work and milestone aggregation endpoints
status: To Do
priority: High
type: Feature
parent: WEB-001
risk: Low
impact: "Adds two read-only endpoints the v2 Ready and Milestones screens need; no write path changes."
tags: [web, tasks, api]
last_updated: 2026-07-14
---

## Summary

The v2 Ready screen needs the same 'To Do with every dependency finished' set the CLI's ready command already computes, and the Milestones screen needs, per milestone-role task, its status, the done/total count of tasks pointing at its milestone value, and those tasks grouped by status. Both are pure reads over an already-validated task list; reuse filer-task's existing readiness and milestone-role logic (do not reimplement dependency-readiness or milestone grouping in the web crate) and add GET /api/ready and GET /api/milestones next to the existing GET /api/tasks handler in routes/tasks.rs.

## Acceptance Criteria

- [ ] GET /api/ready returns exactly the tasks filer-task's ready computation returns for the same project, honoring the same domain/milestone query filters as GET /api/tasks.
- [ ] GET /api/milestones returns one entry per milestone-role task with its status and the done/total count of tasks whose milestone field matches its version.
- [ ] Both endpoints reuse existing filer-task readiness/milestone logic rather than duplicating dependency or grouping computation in filer-task-web.
- [ ] In-process tests cover an empty result, a project with blocked/waiting tasks excluded from ready, and milestone aggregation across several tasks.
