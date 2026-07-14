---
id: WEB-011
title: Build a Settings screen for project and policy management
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-009]
risk: Low
impact: "Adds a screen the v2 mockup never designed: finding or creating a project, and managing its domains/prefixes/task types/tag catalog, from the browser instead of hand-editing config.json."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

Add a Settings screen (new sidebar nav item) with two parts: a 'find or create project' form (path input, an Open button that calls POST /api/projects, and a Create new here checkbox that sets the init flag) and, once a project is open, a policy editor listing its domains/prefixes/task types/tag catalog with add-only forms per section calling PATCH /api/projects/{name}/policy, rendering a rejected removal's blocking-task error inline the same way the New-task form renders field errors.

## Acceptance Criteria

- [ ] The find-or-create form opens an existing project by path or creates a new one, and the newly opened project appears in the sidebar and command palette without a page reload.
- [ ] The policy editor lists current domains, prefixes, task types, and tags and can add one of each through PATCH /api/projects/{name}/policy.
- [ ] A rejected policy change renders its blocking-task error inline on the relevant section, never as a modal.
