---
id: "WEB-020"
title: "Add shared saved views to the Tasks screen"
status: "To Do"
priority: "Low"
type: "Feature"
parent: "WEB-014"
depends_on: ["WEB-015", "WEB-006"]
risk: "Low"
impact: "Adds team-shared saved filter views on top of the v2 filter menu and the storage layer."
tags: ["web", "tasks", "state", "workflow"]
last_updated: "2026-07-14"
---

## Summary

The v2 filter-chip menu (web:WEB-006) rebuilds filters from scratch every visit and nothing shares a useful filter set with the team. Persist named views in the database: a view stores a name and the applied filter set. Add list, create, and delete endpoints and extend the filter menu with a save-current-filters action and a saved-views list that applies a view on click. Views are global; anyone can create or delete one. A view referencing a value that no longer exists renders the same error pills web:WEB-006 defines instead of silently dropping the filter.

## Acceptance Criteria

- [ ] Endpoints list, create, and delete named views persisted in the database, shared by all users.
- [ ] The filter menu can save the currently applied filter set under a name and apply a saved view on click.
- [ ] Applying a view whose stored value no longer exists renders web:WEB-006's error-pill treatment instead of silently dropping that filter.
- [ ] Tests cover the view round-trip, deletion, and applying a view with a stale value.
