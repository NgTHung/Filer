---
id: UTILS-019
title: Refactor task candidate validation and write locking
status: Done
priority: High
type: Refactor
parent: UTILS-005
milestone: "0.3.0"
depends_on: [UTILS-013]
risk: High
impact: "Removes temporary project copying and redundant validation while giving task and policy mutations one stale-write contract."
tags: [tooling, tasks, validation, concurrency]
last_updated: 2026-07-14
---

## Summary

Validate in-memory candidates against the real project, choose local or repository-wide validation by mutation scope, and centralize lock and freshness ownership.

## Acceptance Criteria

- [x] Candidate validation accepts task-content and policy overrides without copying the project tree or creating a temporary project.
- [x] Local field and checklist changes validate only the target while relationship, identity, milestone, and policy changes traverse the repository once.
- [x] Rejected task and policy candidates preserve the original file bytes.
- [x] Ordinary mutations refresh the shared revision for all clones, while policy mutation returns the only fresh handle and leaves old-policy handles stale.
- [x] Focused candidate and lock tests plus all required filer-task checks pass.
