---
id: WEB-005
title: Build the v2 sidebar shell and Ready-work screen
status: To Do
priority: Medium
type: Feature
parent: WEB-001
depends_on: [WEB-002, WEB-003]
risk: Low
impact: "Replaces the v1 static filter bar with the v2 sidebar (project switcher trigger, nav, domain list) and adds the Ready-work screen as the app's default view."
tags: [web, tasks]
last_updated: 2026-07-14
---

## Summary

Rewrite static/index.html, static/app.js, and static/style.css to the v2 layout: a left sidebar with the current project name/switcher button, a nav list (Ready, Tasks, Milestones, New task) with per-item counts, and a domain list beneath it with a blocked-task indicator dot, all driven by GET /api/projects and GET /api/tasks. The main pane's default screen is Ready work: domain/milestone filter chips, a row list from GET /api/ready, an empty state with a reason (X blocked, Y waiting), and a shared header (title, loaded-at, refresh) reused by every other screen added in later tasks. A project that fails validation shows the broken-project screen instead of the sidebar's domain list or nav counts.

## Acceptance Criteria

- [ ] The sidebar lists nav items with live counts and a domain list with per-domain task counts and a blocked-task dot, sourced from real API responses.
- [ ] The Ready screen lists GET /api/ready rows sorted by priority then id, filterable by domain and milestone chips, matching the v2 mockup's empty-state copy pattern.
- [ ] The shared header (screen title, loaded-at label, refresh button) is one reusable piece of markup/JS other screens can mount into, not copy-pasted per screen.
- [ ] Selecting a broken project renders the validation-failure screen (issue list, re-validate and switch-project actions) instead of Ready work.
