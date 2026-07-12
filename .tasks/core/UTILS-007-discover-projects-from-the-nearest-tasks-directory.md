---
id: UTILS-007
title: Discover projects from the nearest .tasks directory
status: To Do
priority: High
type: Feature
parent: UTILS-005
risk: Medium
impact: "Changes project-root selection for every filer-task command and library consumer."
tags: [tooling, tasks, discovery, portability]
last_updated: 2026-07-12
---

## Summary

Resolve a filer-task project from the nearest ancestor containing .tasks. Discovery must work for installed binaries in independent repositories and must not require Filer-specific marker files. Discovery is a CLI convenience layered over explicit-root library operations, so library consumers can open projects directly by path.

## Acceptance Criteria

- [ ] Without --root, every CLI command starts at the current working directory and selects the nearest ancestor that directly contains a .tasks directory.
- [ ] An explicit --root path uses the same ancestor discovery behavior, including paths inside a project, while remaining a deterministic override.
- [ ] A .tasks directory is recognized when task.schema.json is absent; invalid task contents are reported by validation instead of as a missing project.
- [ ] When no .tasks directory exists at or above the start, filer-task exits unsuccessfully with an actionable error containing the searched starting path.
- [ ] Discovery is a standalone library helper, and core task operations accept an explicit project root without invoking discovery or reading the working directory.
- [ ] Integration tests run commands from roots and nested directories in two independent temporary projects and prove their task data remains isolated.
- [ ] Tests cover nearest-project selection for nested projects, explicit --root discovery, and missing-project behavior without relying on the Filer repository.
- [ ] CLI help, public Rust documentation, and task-tracking usage describe automatic discovery, the `--root` override, and missing-project errors without retaining the schema-file requirement.
