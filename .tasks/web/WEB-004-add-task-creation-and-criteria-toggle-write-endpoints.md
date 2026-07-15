---
id: WEB-004
title: Add task creation and conditional criteria updates
status: Done
priority: High
type: Feature
parent: WEB-001
depends_on: [core:UTILS-017]
risk: Medium
impact: "Adds project-scoped task creation and concurrency-safe criterion updates without allowing stale indexes to mutate changed checklist content."
tags: [web, tasks, api]
last_updated: 2026-07-15
---

## Summary

The v2 New-task form creates tasks inside an explicitly selected project and needs field-scoped creation errors. The drawer sets a criterion's absolute checked state. Add `POST /api/projects/{project}/tasks` through filer-task's existing creation path and `PUT /api/projects/{project}/tasks/{qualified_id}/criteria/{index}` through a new conditional library operation. Each serialized checklist item exposes a lowercase SHA-256 hash of its exact source line. Criteria writes require that quoted hash in `If-Match`, so an index cannot silently target content that changed or moved. Both write paths use the shared project lock, validate before and after mutation, and return a refreshed `ShowView`.

## Acceptance Criteria

- [x] `POST /api/projects/{project}/tasks` preserves the number string, creates a `To Do` task through filer-task's existing creation path, and returns the new task's `ShowView`.
- [x] Duplicate IDs, disallowed prefixes, and strict-policy tag rejections return structured errors with codes `id_exists`, `prefix_not_allowed`, and `tag_rejected` and fields `number`, `prefix`, and `tags`.
- [x] Every serialized criterion includes a deterministic lowercase SHA-256 `content_hash` over its exact source line excluding the line terminator.
- [x] `PUT /api/projects/{project}/tasks/{qualified_id}/criteria/{index}` requires a quoted lowercase SHA-256 `If-Match`, sets the requested absolute state, and returns the refreshed `ShowView`.
- [x] Missing, malformed, and mismatched preconditions return 428, 400, and 412 without mutation; a mismatch uses code `criterion_content_mismatch` with expected and actual hashes in context.
- [x] A successful criteria update changes only the selected marker, and an already-requested state skips the file write.
- [x] Creation, lifecycle transitions, and criteria updates share the project write lock, validate before mutation, run blocking work off the async executor, and validate after mutation.
- [x] In-process and library tests cover creation errors, criterion hashes and conditional updates, stale content, project isolation, invalid projects and indexes, and the absence of unscoped write routes.
