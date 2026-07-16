---
id: "WEB-020"
title: "Add shared saved views to the Tasks screen"
status: "To Do"
priority: "Low"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015", "WEB-006"]
risk: "Low"
impact: "Adds team-shared saved filter views on top of the WEB-006 filter menu and the storage layer."
tags: ["web", "tasks", "state", "workflow"]
last_updated: "2026-07-15"
---

## Summary

The filter-chip menu (web:WEB-006) rebuilds filters from scratch every visit and nothing shares a useful filter set with the team. Persist named views in the database: a view stores a name and the applied filter set. Add list, create, and delete endpoints and extend the filter menu with a save-current-filters action and a saved-views list that applies a view on click. Views are global; anyone can create or delete one. A view referencing a value the backend now rejects (a tag dropped from a strict catalog, a parent that became ambiguous) renders web:WEB-006's structured-error pill for that filter instead of silently dropping it.

## Acceptance Criteria

- [ ] Endpoints list, create, and delete named views persisted in the database, shared by all users.
- [ ] The filter menu can save the currently applied filter set under a name and apply a saved view on click.
- [ ] Applying a view whose stored value the backend now rejects renders web:WEB-006's structured-error pill for that filter instead of silently dropping it.
- [ ] Tests cover the view round-trip, deletion, and applying a view with a stale value.
