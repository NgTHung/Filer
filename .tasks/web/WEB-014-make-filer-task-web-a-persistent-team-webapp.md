---
id: "WEB-014"
title: "Make filer-task-web a persistent team webapp"
status: Deferred
priority: "High"
type: "Epic"
risk: "High"
impact: "Turns filer-task-web from a stateless local executable into a self-hosted team webapp with an embedded SQLite layer for registry, identity, activity, index, and saved views, while .tasks/ files remain the source of truth."
tags: ["web", "tasks", "server", "state"]
last_updated: 2026-09-05
---

## Summary

web:WEB-001 rebuilds the UI but keeps filer-task-web a stateless local executable: the project registry lives in memory, nothing survives a restart, and nothing identifies who changed what. This epic turns it into a self-hosted webapp for a small trusted LAN team. The .tasks/ markdown files stay the only source of truth for tasks and the server stays git-unaware; an embedded SQLite database persists webapp state instead: the registered-project list, a self-chosen username per person (no passwords), an attributed activity history of web-driven writes, a task metadata index for fast queries, and named filter views shared by the team.

## Exit Criteria

- [ ] The server embeds a SQLite database it creates and migrates at startup, with no external database service.
- [ ] Registered projects survive a server restart and reappear in the UI without re-registration.
- [ ] Every web-driven write is attributed to a self-chosen username and appears in a queryable activity feed.
- [ ] Task list queries are served from a database index that stays consistent with file changes made outside the webapp.
- [ ] Named filter views persist in the database and are shared across the team.
- [ ] The .tasks/ files remain the only source of truth for task content; deleting the database loses no task data.

## Rationale

The maintainer approved keeping task-web work outside the active filer-core scope on 2026-09-05. Persistence already landed in the completed children; index and saved-view work remain available when this parent is reactivated.
