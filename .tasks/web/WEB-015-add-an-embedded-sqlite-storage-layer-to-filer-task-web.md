---
id: "WEB-015"
title: "Add an embedded SQLite storage layer to filer-task-web"
status: "To Do"
priority: "High"
type: "Feature"
parent: "WEB-014"
risk: "Medium"
impact: "Foundation for every persistence feature in web:WEB-014; introduces the SQLite dependency and the storage module boundary."
tags: ["web", "server", "state"]
last_updated: "2026-07-14"
---

## Summary

Every persistence feature in this epic (registry, identity, activity, index, saved views) needs one storage foundation. Add an embedded SQLite database to filer-task-web: the server opens or creates the database file at a configurable path at startup and applies versioned schema migrations before serving requests. Wrap access in one storage module that exposes typed operations so no other module writes SQL. Choose the SQLite crate deliberately (rusqlite or sqlx) and document the decision; do not add both.

## Acceptance Criteria

- [ ] The server opens or creates the database file at a configurable path and applies pending schema migrations before accepting requests.
- [ ] One storage module owns all SQL; other modules call its typed operations.
- [ ] An unopenable or unmigratable database fails startup with a clear error instead of a panic.
- [ ] Tests cover first-run creation, reopening an existing database, and migrating from an older schema version.
