---
id: "WEB-018"
title: "Record and serve an attributed activity history"
status: "In Progress"
priority: "Medium"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015", "WEB-017", "WEB-004", "WEB-008", "WEB-009", "WEB-010"]
risk: "Low"
impact: "Adds the audit trail for team use: every web write becomes an attributed, queryable activity row with a feed UI."
tags: ["web", "tasks", "audit", "state"]
last_updated: "2026-07-23"
---

## Summary

Nothing records what the team changes through the browser. Record one activity row per successful web-driven write: lifecycle transitions, task creation and criteria toggles (web:WEB-004), field edits (web:WEB-010), and project or policy changes (web:WEB-009), each with timestamp, username, project, task id where applicable, and action detail. Serve GET /api/activity with newest-first pagination and project and task filters, and add an Activity feed screen whose task entries open the detail drawer. A failed write records nothing.

## Acceptance Criteria

- [x] Each successful write endpoint records exactly one activity row with timestamp, username, project, action, and task id where applicable.
- [x] A rejected or failed write records no activity row.
- [x] GET /api/activity returns newest-first pages and filters by project and task.
- [ ] An Activity screen renders the feed and links task entries to the detail drawer.
  - The feed screen is built and renders task ids as plain text. Linking them requires the detail drawer from web:WEB-008, which is still To Do, so there is nothing to link to. Check this once WEB-008 lands.
- [x] Tests cover recording for each write endpoint, a failed write recording nothing, and pagination.
