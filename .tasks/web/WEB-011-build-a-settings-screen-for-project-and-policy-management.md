---
id: WEB-011
title: Build a Settings screen for project and policy management
status: Done
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-009, WEB-021]
risk: Medium
impact: "Adds the screen that finds or creates a project and manages its domains/prefixes/task types/tag catalog from the browser instead of hand-editing config.json."
tags: [web, tasks]
last_updated: 2026-07-27
---

## Summary

Add a Settings screen (new sidebar nav item) with two parts. First, a 'find or create project' form: a path input, an Open button that calls POST /api/projects (web:WEB-009), and a 'Create new here' checkbox that sets the init flag. Second, once a project is open, a policy editor that reads the current domains, prefixes, task types, and tag catalog from GET /api/projects/{project}/policy (web:WEB-021) and offers add and remove controls per section calling PATCH /api/projects/{project}/policy (web:WEB-009). A rejected policy change renders its blocking-task error inline on the relevant section, the same way the New-task form renders field errors.

## Acceptance Criteria

- [x] The find-or-create form opens an existing project by path or creates a new one, and the newly opened project appears in the sidebar and command palette without a page reload.
- [x] The policy editor lists the current domains, prefixes, task types, and tags from GET /api/projects/{project}/policy and can add one of each through PATCH /api/projects/{project}/policy.
- [x] A rejected policy change renders its blocking-task error inline on the relevant section, never as a modal.
- [x] The policy editor removes a domain, a prefix, a task type, and a tag when no task depends on them, and a removal blocked by an existing task renders inline on that section.
- [x] The Settings screen is reachable from a project that fails validation, so a broken project can be swapped without a page reload.
- [x] Adding the first tag to an open-policy project warns that the catalog becomes strict before the request is sent.
