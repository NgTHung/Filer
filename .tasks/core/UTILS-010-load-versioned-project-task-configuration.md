---
id: UTILS-010
title: Load versioned project task configuration
status: Done
priority: High
type: Feature
parent: UTILS-005
depends_on: [UTILS-006]
risk: High
impact: "Introduces project-owned configuration consumed by task loading, validation, creation, and command parsing."
tags: [tooling, tasks, configuration, portability]
last_updated: 2026-07-12
---

## Summary

Load and validate a versioned .tasks/config.json once per command, then pass an immutable project policy through library operations. Projects without the file use documented compatibility defaults.

## Acceptance Criteria

- [x] A typed project-configuration model loads .tasks/config.json and rejects unsupported versions, unknown fields, duplicate entries, invalid names, and conflicting roles with path-aware errors.
- [x] The configuration exposes domain definitions and prefixes, task-type definitions, and tag policy through one shared library API without defining an implicit default domain.
- [x] All commands use one resolved configuration per invocation instead of reopening or reparsing it in validation and lifecycle layers.
- [x] Projects without config.json receive documented defaults that preserve built-in task types, open tags, and compatibility with existing Filer task trees.
- [x] Malformed configuration fails before task reads or writes, and mutating commands never partially update a project after configuration failure.
- [x] Tests cover valid custom configuration, absent configuration, unsupported versions, unknown fields, duplicate entries, invalid domain and prefix names including Windows device names, and filesystem errors.
- [x] Public Rust and configuration documentation includes the config path, version and validation behavior, compatibility defaults, a minimal single-domain project, and a fully customized example.
