---
id: "WEB-024"
title: "Ingest published task snapshots into a mirror data directory"
status: Obsolete
priority: "High"
type: "Feature"
parent: "WEB-022"
depends_on: ["WEB-023"]
risk: "Medium"
impact: "Gives the mirror its write-once entry point; a failed or hostile publish degrades to staleness instead of a broken or escaped project tree."
tags: ["web", "server", "api", "sync"]
last_updated: 2026-08-08
---

## Summary

The mirror receives snapshots over HTTP and must turn them into a project root the existing registry can serve. Add POST /api/ingest/{project} authenticated with a per-project bearer token held in the database. The handler rejects an unknown format version, rejects any relative path that is absolute or escapes the project root, writes the tree into a staging directory beside the live one under a configurable data directory, then swaps it into place atomically and reloads the registry entry. A rejected or failed ingest leaves the previous snapshot serving, so a bad publish costs freshness and nothing else. Store per-project ingest tokens and the last ingest timestamp in a new migration rather than a configuration file, so the mirror keeps the persistence boundary the storage module already owns.

## Acceptance Criteria

- [ ] A valid bundle materializes into the data directory and its project becomes readable through the existing task routes.
- [ ] Ingest rejects an unknown format version, a missing or wrong token, and any absolute or traversing path.
- [ ] A failure partway through materialization leaves the previous snapshot intact and serving.
- [ ] Per-project ingest tokens and last ingest timestamps persist in the database through the storage module.
- [ ] Tests cover a successful ingest, each rejection case, and atomicity under a mid-write failure.

## Rationale

Receiver ingestion and persistence belong to the separate read-only service, not filer-task-web.
