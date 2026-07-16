---
id: WEB-010
title: Serve a task field-edit endpoint
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-004]
risk: Medium
impact: "Backend half of drawer editing: changing an existing task's title, summary, body, risk, impact, tags, milestone, parent, or depends_on after creation."
tags: [web, tasks]
last_updated: 2026-07-15
---

## Summary

The drawer only runs lifecycle transitions and toggles criteria; nothing changes a task's own fields once created. Add PATCH /api/projects/{project}/tasks/{id} over core:UTILS-016's edit_task, accepting a partial patch and returning the refreshed ShowView like the lifecycle transitions do, through the shared mutate helper in routes/write.rs.

## Acceptance Criteria

- [ ] PATCH /api/projects/{project}/tasks/{id} applies a partial patch through core:UTILS-016 and returns the refreshed ShowView.
- [ ] A rejected edit (cyclic parent/depends_on, unknown milestone, disallowed tag) returns a structured error naming the offending field and the original reason code.
- [ ] The endpoint goes through the shared mutate helper in routes/write.rs (per-project write lock, reload, revalidate) like the transition and write routes.
- [ ] In-process tests cover a successful multi-field edit and each rejection reason code.
