---
id: "WEB-026"
title: "Publish task progress to configured external services"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "WEB-022"
depends_on: ["WEB-023"]
risk: "Medium"
impact: "Keeps an independent read-only service current without exposing the local server or coupling Filer to the receiver implementation."
tags: ["web", "protocol", "state", "sync"]
last_updated: "2026-08-08"
---

## Summary

`filer-task-web` already holds the project registry, validates on every write, and owns persistent application state, so it publishes the versioned WEB-023 contract. Store one configured destination URL, bearer token, enabled flag, and poll interval per project, plus the last accepted hash, success time, error, and dirty flag. A completed web mutation, external file-change poll, or on-demand action marks the project dirty. One background worker per project coalesces marks, builds the latest validated snapshot, skips an unchanged hash, and permits only one in-flight PUT. It retries only the transport and status failures named by the contract. Publication never blocks or fails a task write. Filer implements no receiver route, remote storage, read-only hosting mode, or external user interface.

## Acceptance Criteria

- [ ] Each enabled project publishes the WEB-023 payload with HTTP PUT and its bearer credential to the complete configured destination URL.
- [ ] Web writes publish without blocking or failing the write, and polling detects CLI or editor changes; a zero interval disables polling.
- [ ] Dirty marks coalesce, unchanged hashes skip delivery, and each project has at most one in-flight request.
- [ ] Retry follows the documented network, 408, 429, and 5xx rules; persistent errors and accepted hashes are stored and visible in the UI.
- [ ] Target and delivery state stay behind the storage module, and tests use a mock external service to cover authentication headers, hash acknowledgement, retry, coalescing, and external-edit detection.
