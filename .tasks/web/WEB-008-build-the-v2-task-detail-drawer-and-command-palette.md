---
id: WEB-008
title: Build the v2 task detail drawer and command palette
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-004]
risk: Medium
impact: "Replaces the v1 inline detail panel with the v2 right-side drawer and adds the cmd-K project switcher, the last two pieces of the v2 design."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

Task detail drawer: header (qualified id, status, priority, close), title, Start/Done/Block/Defer/Obsolete actions. Block/Defer/Obsolete open an inline reason form (confirm disabled until non-empty) that posts to the existing reason-required transition endpoints. Done against a task with unchecked criteria does not call the API; it shows a 'done_refused' banner naming the unchecked count and highlights the unchecked rows, matching the mockup's client-side refusal. Body: type/milestone/updated/risk/impact/tags grid, parent chain, children/dependencies/dependents relationship chips (all clickable to reselect), a blocked-by chain for Blocked tasks, summary, and an acceptance/exit criteria list wired to POST /api/tasks/{id}/criteria/{index} from WEB-004. Command palette: cmd-K opens a modal fed by GET /api/projects, arrow-key navigable, enter to switch project, escape to close.

## Acceptance Criteria

- [ ] The drawer's five lifecycle actions call the corresponding existing /api/tasks/{id}/... endpoint and refresh the drawer from the response.
- [ ] Clicking Done with unchecked criteria shows the refusal banner and never calls the done endpoint; checking every item first allows Done to succeed.
- [ ] Parent chain, children, dependencies, dependents, and blocked-by chips are each clickable and reselect the drawer to that task.
- [ ] Cmd/Ctrl-K opens the palette, arrow keys move selection, enter switches the active project, and escape closes it.
- [ ] Toggling an acceptance/exit criteria item persists through POST /api/tasks/{id}/criteria/{index} and survives a refresh.
