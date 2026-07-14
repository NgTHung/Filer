---
id: WEB-002
title: Discover, list, and switch between task-web projects
status: Done
priority: High
type: Feature
parent: WEB-001
risk: Medium
impact: "Replaces the single-project ProjectRegistry with one that discovers several .tasks projects, so the UI can switch projects and surface one that fails validation without disabling the others."
tags: [web, tasks, discovery]
last_updated: 2026-07-14
---

## Summary

ProjectRegistry::single only ever opens the one project nearest a start path. The v2 design's command palette (cmd-K) lists every known project, shows a validation_failed badge on a broken one, and switches the active project on click. Extend the registry to accept a list of project roots (web:WEB-016 later persists that list in the database), derive each public name from its canonical root directory, open each with TaskProject::open, and keep a project whose validate_repo report has errors registered with those issues. Expose GET /api/projects with name, task count, domain count, and broken/issues. Move task reads and transitions to /api/projects/{project}/tasks routes and remove the unscoped routes. The touched handlers must also be updated to the current filer-task typed-identity, graph, warning, and error APIs so the crate compiles and tests against its library dependency.

## Acceptance Criteria

- [x] ProjectRegistry can be constructed from more than one root, derives names from canonical root directory basenames, rejects duplicate names, and keeps a project whose .tasks/ fails validate_repo registered.
- [x] GET /api/projects returns each project's name, task count, domain count, broken flag, and (when broken) validation issues with code, path, message, and context.
- [x] Task reads and transitions use /api/projects/{project}/tasks routes, and the previous unscoped /api/tasks routes are removed.
- [x] Resolving tasks or transitions against a broken project returns a clear WebError instead of a panic or stale data, while other projects keep working.
- [x] The touched web handlers use the current filer-task typed-identity, graph-aware filtering, validation-warning, and error APIs.
- [x] Tests cover multi-project construction, duplicate names, one broken project not affecting the others, project-prefixed reads and transitions, removed unscoped routes, and the /api/projects response shape.
