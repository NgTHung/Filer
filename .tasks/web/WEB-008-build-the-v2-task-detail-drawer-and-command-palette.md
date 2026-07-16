---
id: WEB-008
title: Build the task detail drawer and command palette
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-004, WEB-005]
risk: Medium
impact: "Replaces the old static detail panel and window.prompt transitions with a right-side drawer wired to the project-scoped transition and criteria endpoints, and adds the cmd-K project switcher."
tags: [web, tasks]
last_updated: 2026-07-15
---

## Summary

Task detail drawer over GET /api/projects/{project}/tasks/{id}: render the ShowView detail (task fields, criteria, sections) the old panel already consumed, plus header (qualified id, status, priority, close), parent chain, children/dependencies/dependents relationship chips (clickable to reselect), and a blocked-by chain for Blocked tasks. Lifecycle actions post to the five existing project-scoped endpoints (start, done, block, defer, obsolete) and refresh the drawer from the returned ShowView. Block, Defer, and Obsolete open an inline reason form with confirm disabled until non-empty, replacing the current window.prompt flow. Done against unchecked criteria never calls the API: it shows a done_refused banner naming the unchecked count and highlights the unchecked rows. Criteria items toggle through PUT /api/projects/{project}/tasks/{id}/criteria/{index} with an If-Match header carrying the criterion's content_hash from the ShowView; a 412 (content mismatch) or 428 (missing precondition) response refreshes the drawer and surfaces a conflict notice instead of retrying. Command palette: cmd/ctrl-K opens a modal fed by GET /api/projects, arrow-key navigable, enter switches the active project (rescoping every API path), escape closes.

## Acceptance Criteria

- [ ] The drawer's five lifecycle actions call the existing POST /api/projects/{project}/tasks/{id} transition endpoints and refresh the drawer from the returned ShowView, with inline reason forms for block, defer, and obsolete.
- [ ] Clicking Done with unchecked criteria shows the refusal banner and never calls the done endpoint; checking every item first allows Done to succeed.
- [ ] Toggling a criterion sends PUT /api/projects/{project}/tasks/{id}/criteria/{index} with If-Match set to that criterion's content_hash and persists across a refresh.
- [ ] A 412 or 428 response to a criteria toggle refreshes the drawer and surfaces a conflict notice instead of silently retrying.
- [ ] Parent chain, children, dependencies, dependents, and blocked-by chips are each clickable and reselect the drawer to that task.
- [ ] Cmd/Ctrl-K opens the palette, arrow keys move selection, enter switches the active project and rescopes API paths, and escape closes it.
