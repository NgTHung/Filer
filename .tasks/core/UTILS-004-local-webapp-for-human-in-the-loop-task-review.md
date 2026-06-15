---
id: UTILS-004
title: Local webapp for human-in-the-loop task review
status: Done
priority: Medium
type: Feature
risk: Medium
impact: "Adds a localhost UI over filer-task so a human can review, filter, and transition tasks without editing markdown."
tags: [tooling, web, tasks]
last_updated: 2026-06-15
---

## Summary

A new filer-task-web binary crate serves a localhost-only HTTP API plus static UI over the existing filer-task library, with no forking of model/validate/lifecycle logic. It closes the human-in-the-loop gap: browse and filter tasks, view detail, and run the existing state transitions from a browser. The project-root resolution is isolated behind a registry so reading multiple .tasks/ directories can be added later without changing filer-task signatures. Out of scope for this task: reprioritize, context/ready/summary endpoints, multi-project routing, and body/criteria editing.

## Acceptance Criteria

- [x] A filer-task-web crate serves an HTTP API bound to 127.0.0.1 only, built on the filer-task library without duplicating model, validate, or lifecycle logic.
- [x] GET /api/tasks returns validated tasks and honors status, priority, domain, milestone, and tag filters.
- [x] GET /api/tasks/:id returns full task detail and a missing id returns 404.
- [x] start, done, block, defer, and obsolete are exposed as endpoints; each validates before writing and returns the refreshed task detail.
- [x] A static web UI lists and filters tasks, opens a task detail view, and triggers each transition.
- [x] In-process API tests cover listing, filtering, detail, the 404 case, and each transition; filer-task validate passes after web-driven writes.
