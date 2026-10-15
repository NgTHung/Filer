---
id: "UTILS-023"
title: "Publish tasks to GitHub Projects with one-way sync"
status: Done
priority: "Medium"
type: "Feature"
tags: ["cli", "tasks", "sync"]
last_updated: 2026-10-15
---

## Summary

Expose a one-way taskroot command that previews or publishes validated tasks as GitHub Project draft items. Keep .tasks authoritative, avoid duplicate drafts on reruns, and prepare an opt-in workflow for Filer. Validate through command tests against a fake GitHub API; skip live GitHub writes at the maintainer's request.

## Verification

`cargo test -p taskroot --all-features`, taskroot Clippy with warnings denied,
workspace formatting, and task validation pass. The 14 GitHub command tests
cover preview, field mapping, unchanged reruns, pagination, duplicate markers,
invalid local tasks, API errors, partial publication recovery, field clearing,
and archived, converted, or inaccessible items.

A preview against this checkout through the fake GitHub boundary selects all
226 tasks, performs zero GitHub mutations, and leaves task files byte-identical.
The workflow YAML parses and its shell script passes `bash -n`. Live GitHub
writes and workflow activation are excluded at the maintainer's request.

## Acceptance Criteria

- [x] A public github-sync command validates the source Task project and previews changes without GitHub writes by default.
- [x] Explicit apply creates or updates owned draft items and mapped project fields; repeated runs create no duplicates and skip unchanged values.
- [x] Command tests cover invalid sources, pagination, duplicate ownership, partial failures, and GitHub GraphQL errors without live writes.
- [x] Filer has documented setup and a disabled-by-default workflow using a separate Projects credential; local checks pass.
