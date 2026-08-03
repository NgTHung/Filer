---
id: "WEB-028"
title: "Create a project through a name-and-location dialog"
status: Done
priority: "Medium"
type: "Feature"
parent: "WEB-001"
depends_on: ["web:WEB-027"]
risk: "Medium"
impact: "Makes creating a project an explicit, named choice instead of overloading the switcher query as a filesystem path."
tags: ["web", "tasks"]
last_updated: 2026-08-02
---

## Summary

The switcher's create action (web:WEB-027) registers whatever text was typed as the project path, which is both an awkward thing to type into a filter box and unable to name the project: the registry name is the root directory's own name. Open a modal from the create action instead, asking for a project name and the directory to create it in, and extend POST /api/projects so an init request may carry a name, creating that directory under the given path before initializing it. A name that is empty or carries a path separator is refused with a field-scoped error the dialog renders on the offending input.

## Acceptance Criteria

- [x] POST /api/projects with init and a name creates the named directory under the given path, initializes it, and registers it under that name.
- [x] An init request whose named directory already exists initializes it in place rather than failing, and a name that is empty or holds a path separator is refused with a field-scoped 422.
- [x] A named directory that already holds a project registers that project instead of failing, so the server owns find-or-create for the root it builds.
- [x] A name is only meaningful with init: a request that sends one without the init flag is refused rather than silently ignored.
- [x] The switcher's create action opens a modal with a name field prefilled from the query and a directory field, and creating from it activates the new project and closes both the modal and the switcher.
- [x] A refused creation keeps the modal open and renders the message on the offending field.
- [x] Cancelling the modal, by its Cancel button, a click outside it, or Escape, returns to the switcher instead of closing it.
