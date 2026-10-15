---
id: "UTILS-023"
title: "Publish tasks to GitHub Projects with one-way sync"
status: In Progress
priority: "Medium"
type: "Feature"
tags: ["cli", "tasks", "sync"]
last_updated: 2026-10-15
---

## Summary

Expose a one-way taskroot command that previews or publishes validated tasks as GitHub Project draft items. Keep .tasks authoritative, avoid duplicate drafts on reruns, and prepare an opt-in workflow for Filer. Validate through command tests against a fake GitHub API; skip live GitHub writes at the maintainer's request.

## Acceptance Criteria

- [ ] A public github-sync command validates the source Task project and previews changes without GitHub writes by default.
- [ ] Explicit apply creates or updates owned draft items and mapped project fields; repeated runs create no duplicates and skip unchanged values.
- [ ] Command tests cover invalid sources, pagination, duplicate ownership, partial failures, and GitHub GraphQL errors without live writes.
- [ ] Filer has documented setup and a disabled-by-default workflow using a separate Projects credential; local checks pass.
