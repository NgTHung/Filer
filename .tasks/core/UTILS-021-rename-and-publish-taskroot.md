---
id: "UTILS-021"
title: "Rename and publish Taskroot"
status: Done
priority: "High"
type: "Feature"
milestone: "0.3.0"
risk: "Medium"
impact: "Creates the public Taskroot package and command while preserving the existing task-project storage contract."
tags: ["tooling", "tasks", "cli", "library", "portability"]
last_updated: 2026-09-05
---

## Summary

Rename the reusable task project library and CLI from filer-task to Taskroot, complete the package metadata and release documentation, verify the crate archive as an independent dependency and installed command, then publish the validated release to crates.io.

## Acceptance Criteria

- [x] The crates.io package, Rust library, and installed executable use the taskroot name while .tasks remains the project marker.
- [x] Cargo metadata names the release documentation, repository, license, keywords, categories, and supported Rust version.
- [x] The packaged crate builds and its installed executable initializes and validates a fresh independent project.
- [x] Workspace consumers compile against the renamed Taskroot library without duplicating task logic.
- [x] Current user and agent documentation uses the published taskroot command and explains migration from filer-task.
- [x] The validated Taskroot release is published to crates.io.

## Completion evidence

The maintainer confirmed on 2026-09-05 that the validated Taskroot release was published to crates.io. The earlier package and consumer checks remain recorded in commit bc2bd8b.
