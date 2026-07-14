---
id: UTILS-018
title: Harden task initialization, YAML rendering, and checklist mutation
status: Done
priority: High
type: Bug
parent: UTILS-005
milestone: "0.3.0"
depends_on: [UTILS-013]
risk: High
impact: "Prevents failed initialization from leaving partial projects and prevents task creation or edits from corrupting YAML or checklist state."
tags: [tooling, tasks, validation]
last_updated: 2026-07-14
---

## Summary

Make project initialization transactional, centralize YAML-safe frontmatter rendering, and use one checklist parser for reads and toggles.

## Acceptance Criteria

- [x] Invalid domains and empty, invalid, or duplicate prefixes fail before .tasks is created, and initialization can be retried with valid options.
- [x] Every failure after initialization creates files removes only those files and removes .tasks only when it is empty.
- [x] Created and edited string frontmatter values round-trip YAML-sensitive characters and newlines exactly.
- [x] Checklist reads and toggles share one matcher for indentation, case, malformed markers, and CRLF input.
- [x] Focused initialization, YAML, and checklist tests pass.
