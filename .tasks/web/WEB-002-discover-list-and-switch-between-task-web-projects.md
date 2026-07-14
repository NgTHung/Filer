---
id: WEB-002
title: Discover, list, and switch between task-web projects
status: To Do
priority: High
type: Feature
parent: WEB-001
risk: Medium
impact: "Replaces the single-project ProjectRegistry with one that discovers several .tasks projects, so the UI can switch projects and surface one that fails validation without disabling the others."
tags: [web, tasks, discovery]
last_updated: 2026-07-14
---

## Summary

ProjectRegistry::single only ever opens the one project nearest a start path. The v2 design's command palette (cmd-K) lists every known project, shows a validation_failed badge on a broken one, and switches the active project on click. Extend the registry to accept a list of project roots (web:WEB-016 later persists that list in the database; this task keeps it in memory), open each with TaskProject::open, keep broken ones registered with their filer-task validate issues instead of failing registry construction, and expose GET /api/projects with name, task count, domain count, and broken/issues so the frontend can render the palette without a second round trip per project.

## Acceptance Criteria

- [ ] ProjectRegistry can be constructed from more than one root and keeps every project registered, including one whose .tasks/ fails validate_repo.
- [ ] GET /api/projects returns each project's name, broken flag, and (when broken) its validation issues in the same shape filer-task validate produces.
- [ ] Resolving tasks or transitions against a broken project returns a clear WebError instead of a panic or stale data, while other projects keep working.
- [ ] Unit tests cover multi-project construction, one broken project not affecting resolution of the others, and the /api/projects response shape.
