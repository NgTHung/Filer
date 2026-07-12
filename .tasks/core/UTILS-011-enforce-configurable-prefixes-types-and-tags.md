---
id: UTILS-011
title: Enforce configurable prefixes types and tags
status: To Do
priority: High
type: Feature
parent: UTILS-005
depends_on: [UTILS-008]
risk: High
impact: "Replaces compile-time prefix and type catalogs and adds optional strict tag validation across reads and writes."
tags: [tooling, tasks, configuration, validation]
last_updated: 2026-07-12
---

## Summary

Apply project configuration to task parsing, validation, creation, import, filtering, and output. Prefixes are scoped to domains, task types carry behavior instead of relying on a closed enum, and tags may remain open or use a strict catalog.

## Acceptance Criteria

- [ ] ID validation reads allowed prefixes from the task domain configuration, and the same prefix may be configured independently in multiple domains.
- [ ] Task metadata and CLI parsing accept configured task types without recompilation while retaining a stable representation in human and JSON output.
- [ ] Each task type declares acceptance-criteria or exit-criteria behavior, and an optional milestone role drives milestone-specific validation and commands without checking a hardcoded type name.
- [ ] Open tag policy accepts any syntactically valid tag, while strict policy rejects tags outside the configured catalog during validate, add, and import.
- [ ] Configuration changes are applied consistently to existing task files and new writes, with errors naming the domain, field, and rejected value.
- [ ] Filer receives an explicit project configuration that preserves its current prefixes, built-in types, and existing tags before hardcoded catalogs are removed.
- [ ] Tests cover custom prefixes by domain, custom normal and container types, a renamed milestone type, open tags, strict tags, and unchanged legacy behavior.
- [ ] CLI help, examples, task-tracking documentation, and public Rust documentation explain configuration fields, validation behavior, custom type roles, tag policies, and safe taxonomy migration without retaining hardcoded catalogs.
