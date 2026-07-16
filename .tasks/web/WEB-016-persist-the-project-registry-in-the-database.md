---
id: "WEB-016"
title: "Persist the project registry in the database"
status: Done
priority: "High"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015", "WEB-002", "WEB-009"]
risk: "Medium"
impact: "Makes the multi-project registry survive restarts, replacing web:WEB-002's in-memory root list as the deployment-facing configuration."
tags: ["web", "tasks", "discovery", "state"]
last_updated: 2026-07-16
---

## Summary

web:WEB-002 builds the multi-project registry from an in-memory list of roots and web:WEB-009 registers new projects into it, but the set is lost on restart. Load the registry from the database at startup: each registration row stores the project root, and the server reopens each root with the same broken-project tolerance web:WEB-002 defines. POST /api/projects writes a registration row, and a delete endpoint removes one. A persisted root whose directory no longer exists surfaces as a broken project instead of failing startup.

## Acceptance Criteria

- [x] The registry loads persisted roots at startup and reopens each project, keeping broken ones visible per web:WEB-002.
- [x] A project registered through POST /api/projects is still registered after a server restart.
- [x] A persisted root that no longer exists on disk appears as a broken project and does not prevent startup.
- [x] DELETE /api/projects/{project} removes the registration row and the project disappears from GET /api/projects.
- [x] Tests cover the restart round-trip, a vanished root, and deregistration.
