---
id: WEB-007
title: Build the Milestones screen and New-task form
status: In Progress
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-003, WEB-004, WEB-005, WEB-021]
risk: Low
impact: "Adds the two screens the old UI never had: a milestone progress view over the aggregation endpoint and an in-app creation form over the existing POST endpoint, closing the human-in-the-loop gap for creation."
tags: [web, tasks]
last_updated: 2026-07-25
---

## Summary

Milestones screen: one card per GET /api/projects/{project}/milestones entry, rendering the aggregation's milestone status, a progress bar from its done/total count, its criteria checklist, and its tasks grouped by status. New-task screen: a form whose fields mirror CreateTaskRequest exactly (domain, prefix, number, title, type, priority, optional milestone, tags), submitting to POST /api/projects/{project}/tasks. The domain, prefix, task type, and tag options come from GET /api/projects/{project}/policy (web:WEB-021): prefix options are scoped to the chosen domain, tags are a catalog picker under strict policy and free text under open policy. The number field defaults to one past the highest existing number for the chosen domain and prefix, with a live preview of the qualified id and file path. A rejected creation renders the structured error next to the field it names (the error body maps id_exists to number, prefix_not_allowed to prefix, tag_rejected to tags), never as a modal or toast. Success opens the new task's detail drawer using the ShowView the endpoint returns.

## Acceptance Criteria

- [ ] The Milestones screen renders one card per GET /api/projects/{project}/milestones entry with its progress bar, criteria checklist, and tasks grouped by status.
- [ ] The New-task form's domain, prefix, type, and tag options come from GET /api/projects/{project}/policy, with prefix options scoped to the chosen domain and the tag input strict-or-open per the policy.
- [ ] The number field defaults to one past the highest existing number for the chosen domain and prefix, with a live preview of the qualified id and file path.
- [ ] A rejected POST /api/projects/{project}/tasks renders its structured error inline next to the field named by the error body's field mapping (id_exists to number, prefix_not_allowed to prefix, tag_rejected to tags).
- [ ] A successful submission opens the new task's detail drawer from the returned ShowView.
