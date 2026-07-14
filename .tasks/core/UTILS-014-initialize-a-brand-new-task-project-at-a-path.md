---
id: UTILS-014
title: Initialize a brand-new task project at a path
status: To Do
priority: High
type: Feature
parent: UTILS-013
risk: Medium
impact: "Adds the only entry point that can produce a new .tasks/config.json; every other library function assumes one already exists."
tags: [tasks, library, configuration]
last_updated: 2026-07-14
---

## Summary

TaskProject::open (project.rs) and discover_project_root (repo.rs) both require an existing .tasks/ directory; nothing in the crate can create one. Add TaskProject::init(root, policy) that fails if a .tasks/ already exists at that root, writes a minimal valid config.json (version 1, an empty or caller-supplied starting domain), and returns an opened TaskProject — no domain directories are created until the first task is added to one. Wire a filer-task init CLI subcommand on top.

## Acceptance Criteria

- [ ] TaskProject::init fails with a clear error if .tasks/ already exists at the target root, and never partially writes config.json on failure.
- [ ] TaskProject::init accepts an optional starting domain/prefix set and writes a config.json that filer-task validate accepts with zero tasks.
- [ ] A new filer-task init CLI subcommand calls the same library function and documents its flags in docs/task-tracking.md.
- [ ] Tests cover init on an empty directory, init failing when .tasks/ already exists, and the resulting project passing validate_repo with no tasks.
