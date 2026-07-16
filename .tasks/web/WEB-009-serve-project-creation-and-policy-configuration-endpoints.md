---
id: WEB-009
title: Serve project-creation and policy-configuration endpoints
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002]
risk: Medium
impact: "Backend half of the Settings screen: creating/finding a project by path and editing its domain/prefix/type/tag policy from the browser."
tags: [web, tasks, configuration]
last_updated: 2026-07-15
---

## Summary

The command palette only switches between already-registered projects; nothing points the app at a new .tasks/ path or creates one, and no endpoint edits config.json. On top of core:UTILS-014 (init) and core:UTILS-015 (policy mutation), add POST /api/projects to register an existing project found at a path or (with a flag) initialize a brand-new one there, and PATCH /api/projects/{project}/policy to add a domain, prefix, task type, or tag. Both writes go through the shared mutate pattern's per-project locking where applicable. Register successfully-added projects into the running ProjectRegistry from WEB-002 without a restart.

## Acceptance Criteria

- [ ] POST /api/projects with an existing .tasks/ path adds it to the registry and returns its GET /api/projects entry; a path with no .tasks/ and no init flag returns a clear not-found error.
- [ ] POST /api/projects with an init flag calls core:UTILS-014 and registers the newly created project.
- [ ] PATCH /api/projects/{project}/policy adds a domain, prefix, task type, or tag through core:UTILS-015 and returns a structured error, naming the blocking task, for a rejected removal.
- [ ] A project added or edited through these endpoints is immediately visible to GET /api/projects and GET /api/projects/{project}/tasks without restarting the server.
- [ ] In-process tests cover registering an existing project, initializing a new one, a rejected path, and a rejected policy removal.
