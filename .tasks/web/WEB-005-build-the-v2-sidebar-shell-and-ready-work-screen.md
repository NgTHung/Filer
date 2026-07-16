---
id: WEB-005
title: Build the app shell, sidebar, and Ready-work screen
status: Done
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-003]
risk: Medium
impact: "Replaces the broken single-project frontend with a project-scoped app shell (vendored Preact + htm, no build step) whose default view is the Ready-work screen; every later frontend task builds on this shell."
tags: [web, tasks]
last_updated: 2026-07-16
---

## Summary

The current frontend is dead code: static/app.js fetches unscoped /api/tasks URLs the server removed (the api_test regression test asserts they 404), and it has no way to supply the {project} segment every real endpoint requires. Replace static/index.html, static/app.js, and static/style.css with a project-scoped app shell. Vendor Preact and htm as local files under static/vendor/ and load them as native ES modules; no bundler, since the server serves static/ straight from disk. Establish the shared plumbing once: a fetch wrapper that parses the error body shape from src/error.rs (error, code, field, context, issues), an active-project store that prefixes every API path with /api/projects/{project}, a shared screen header (title, loaded-at, refresh), and a module layout (api, components, screens) instead of one growing app.js. On load, pick the active project from GET /api/projects. The sidebar shows the project name with a switcher trigger, a nav list (Ready, Tasks, Milestones, New task) with per-item counts, and a domain list with per-domain counts and a blocked-task dot. The default screen is Ready work: rows from GET /api/projects/{project}/ready with domain and milestone filter chips (that endpoint's actual query params) and an empty state naming why nothing is ready. A project whose ProjectSummary has broken: true, or a request answered with the 422 ProjectBroken body, renders the validation-failure screen (issue list, re-validate and switch-project actions) instead of Ready work.

## Acceptance Criteria

- [x] Preact and htm are vendored under static/vendor/ and loaded as native ES modules; the app is split into api, components, and screens modules with no build step.
- [x] Every API call goes through one fetch wrapper that scopes paths to the active project and parses the structured error body (error, code, field, context, issues).
- [x] The sidebar lists nav items with live counts and a domain list with per-domain task counts and a blocked-task dot, sourced from GET /api/projects and GET /api/projects/{project}/tasks.
- [x] The Ready screen lists GET /api/projects/{project}/ready rows sorted by priority then id, filterable by domain and milestone chips, with an empty state naming why nothing is ready.
- [x] The shared header (screen title, loaded-at label, refresh button) is one reusable component other screens mount, not copy-pasted per screen.
- [x] Selecting a project with broken: true, or receiving the 422 ProjectBroken response, renders the validation-failure screen (issue list, re-validate and switch-project actions) instead of Ready work.
