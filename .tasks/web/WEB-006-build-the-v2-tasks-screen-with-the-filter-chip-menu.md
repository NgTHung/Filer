---
id: WEB-006
title: Build the Tasks screen with the filter-chip menu
status: Done
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-005, WEB-021]
risk: Medium
impact: "Replaces the old dropdown filter bar with a policy-aware filter-chip menu over the real list endpoint's query params, on top of the WEB-005 shell."
tags: [web, tasks]
last_updated: 2026-07-23
---

## Summary

Add the Tasks screen to the WEB-005 shell: a sortable table (id, title, status, priority, type, milestone, updated) over GET /api/projects/{project}/tasks, and a '+ Filter' menu popover exposing that endpoint's actual query params: status, priority, domain, parent, milestone, tag, blocked, and sort_by. The tag input is policy-aware via GET /api/projects/{project}/policy (web:WEB-021): under a strict tag policy it is a picker over the catalog, under an open policy it is free text. The parent filter prefers qualified domain:LOCAL-ID values; when the backend rejects a filter (tag_rejected under strict policy, or an ambiguous bare parent id, both from web:WEB-021), the rejection renders as an inline error pill built from the structured error body, with clickable candidate chips when the error context carries candidates. A 'Blocked only' toggle maps to the blocked query param. Column sorting uses the sort_by param for the columns the endpoint can sort (id, status, priority) and keeps that server order; the remaining columns have no query param and sort in the client. The empty state offers a clear-filters action.

## Acceptance Criteria

- [x] The filter menu applies status, priority, domain, parent, milestone, tag, and blocked filters as GET /api/projects/{project}/tasks query params without a full page reload.
- [x] The tag filter is a catalog picker when the policy from GET /api/projects/{project}/policy is strict and free text when it is open.
- [x] A backend filter rejection (tag_rejected, ambiguous parent reference) renders as an inline error pill from the structured error body, with clickable candidates when the context provides them, and is excluded from the applied filter set.
- [x] Sortable column headers toggle ascending/descending and show the active sort direction; id, status and priority sort through the sort_by query param without a client re-sort, and the other columns sort in the client.
- [x] The empty state offers a clear-filters action that resets every applied filter.
