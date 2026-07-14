---
id: UTILS-017
title: Toggle a single acceptance or exit criteria item by index
status: To Do
priority: Medium
type: Feature
parent: UTILS-013
risk: Low
impact: "Lets a consumer check or uncheck one checklist item without rewriting the rest of the task file, needed by the web UI's criteria list and useful for the CLI."
tags: [tasks, library]
last_updated: 2026-07-14
---

## Summary

No function in the crate touches the '## Acceptance Criteria' / '## Exit Criteria' checklist after task creation. Add toggle_criterion(project, id, index) that flips exactly one [ ]/[x] marker by its zero-based position in the list and rewrites only that line, leaving every other section of the file byte-identical. Out of range index is a clear error, not a panic.

## Acceptance Criteria

- [ ] toggle_criterion flips exactly one checklist marker and the rest of the file is byte-identical before and after.
- [ ] An out-of-range index returns a structured error instead of panicking.
- [ ] The write is atomic, matching the guarantee core:UTILS-004 established for task file writes.
- [ ] Tests cover toggling the first, last, and an out-of-range index, and confirm re-toggling restores the original file bytes.
