---
id: "WEB-015"
title: "Add an embedded SQLite storage layer to filer-task-web"
status: Done
priority: "High"
type: "Feature"
parent: "WEB-014"
risk: "Medium"
impact: "Foundation for every persistence feature in web:WEB-014; introduces the SQLite dependency and the storage module boundary."
tags: ["web", "server", "state"]
last_updated: 2026-07-14
---

## Summary

Every persistence feature in this epic (registry, identity, activity, index, saved views) needs one storage foundation. Add an embedded SQLite database to filer-task-web with SQLx 0.9. The async pool fits the Tokio server, bundles SQLite, and tracks embedded migrations without a second database dependency. The server accepts `--database <path>`, defaulting to `filer-task-web.sqlite3` in the current directory, then opens or creates that file and applies migrations before binding the listener. A private four-connection pool uses WAL mode, foreign keys, full synchronous durability, and a five-second busy timeout. The storage module exposes typed operations, initially schema version and close, so downstream modules never access the pool or write SQL outside this boundary. SQLx migration metadata is the schema-version source; the foundation migration establishes version 1 without creating tables owned by later tasks.

## Acceptance Criteria

- [x] The server opens or creates the database file at a configurable path and applies pending schema migrations before accepting requests.
- [x] One storage module owns all SQL; other modules call its typed operations.
- [x] An unopenable or unmigratable database fails startup with a clear error instead of a panic.
- [x] Tests cover first-run creation, reopening an existing database, and migrating from an older schema version.
