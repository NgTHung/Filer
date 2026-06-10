---
id: UTILS-002
title: Normalize filer-task human output
status: Done
priority: Medium
type: Refactor
risk: Medium
impact: "Changes human-readable output across every filer-task subcommand."
tags: [cli, output]
last_updated: 2026-06-06
---

## Summary

Route successful human-readable command output through one tested rendering boundary.

## Acceptance Criteria

- [x] Every successful subcommand uses shared human output rendering.
- [x] Lifecycle and import output use repo-relative forward-slash paths.
- [x] Existing JSON output, stderr errors, and exit codes remain unchanged.
- [x] Tests cover normalized output for read, validation, import, and lifecycle commands.
