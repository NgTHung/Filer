---
id: "WEB-027"
title: "Create a project from the switcher when nothing matches"
status: Done
priority: "Medium"
type: "Feature"
parent: "WEB-001"
depends_on: ["web:WEB-009"]
risk: "Low"
impact: "Removes the detour through Settings when the project you want is not registered yet: the switcher itself can create it."
tags: ["web", "tasks"]
last_updated: 2026-08-02
---

## Summary

The Cmd/Ctrl-K switcher (web:WEB-008) can only pick an already-registered project; a query that matches nothing renders a dead-end note, and registering a project is only reachable from the Settings screen's open form (web:WEB-011). Offer a create action in the switcher when the query matches no project, treating the typed text as a project path and registering it through POST /api/projects with the init flag (web:WEB-009). The find-or-create handling that Settings already implements moves into a shared module so both callers register, activate, and report rejections the same way.

## Acceptance Criteria

- [x] A query matching no registered project renders a selectable create action in place of the dead-end note, and an empty query never does.
- [x] Choosing the create action registers the typed path with the init flag, makes the new project active, and closes the switcher.
- [x] A rejected creation keeps the switcher open and renders the server's message in the panel.
- [x] A typed path that resolves to an already-registered project switches to it instead of reporting a failure, matching the Settings form.
- [x] Asking to create at a path that already holds an unregistered project opens that project instead of failing, for the switcher and the Settings form alike.
- [x] Settings and the switcher share one find-or-create module rather than duplicating the registration and activation sequence.
