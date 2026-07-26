---
id: "WEB-026"
title: "Publish snapshots from the local server on write and on a poll"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "WEB-022"
depends_on: ["WEB-023", "WEB-024"]
risk: "Medium"
impact: "Removes the manual publish step and catches agent and CLI edits the server never witnesses; the only stage that adds a dependency."
tags: ["web", "server", "state", "sync"]
last_updated: "2026-07-26"
---

## Summary

filer-task-web already holds the project registry, validates a project on every write, and owns a database, which makes it the natural publisher. Add a migration with a mirror target table holding project name, mirror URL, token, an enabled flag, and a poll interval, plus a publish state table holding the last content hash, last publish time, last error, and a dirty flag. Keep one row per project rather than a queue, because every publish sends the complete snapshot and a later one supersedes an earlier one. Three triggers mark a project dirty: a completed write in the shared mutate path, a per-project poll ticker where a zero interval disables polling, and an on-demand publish endpoint. A background task drains dirty projects, builds the bundle, skips the push when the content hash matches the last published one, and retries with backoff. Publishing never blocks or fails a write; errors land in the publish state and surface in the UI. The poll ticker stats the task tree and hashes contents only when a modification time or size moved, which is what catches edits made by agents and the CLI. Publishing needs an HTTP client, so add reqwest with rustls-tls and gzip and no default features. The mirror cannot pull instead because the publishing server binds loopback behind NAT.

## Acceptance Criteria

- [ ] A web-driven write publishes the affected project without blocking or failing that write.
- [ ] The poll loop detects a task file edited outside the server and publishes it; a zero interval disables polling for that project.
- [ ] Repeated dirty marks coalesce into one push, and a push is skipped when the content hash is unchanged.
- [ ] A failed push retries with backoff and its error is stored and visible in the UI.
- [ ] Mirror targets and publish state persist through the storage module, and tests cover coalescing, hash skipping, retry, and external-edit detection.
