---
id: API-004
title: Remove NodeId completely
status: To Do
priority: High
type: Refactor
parent: CORE-001
milestone: "0.3.0"
depends_on: [CORE-025]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Removes NodeId from public and internal core contracts instead of preserving compatibility surfaces."
tags: [api, nodeid, location, compatibility]
last_updated: 2026-07-05
---

## Summary

Remove NodeId from filer-core contracts and implementation entirely. Do not preserve deprecated routes, compatibility event variants, or translation paths, because keeping them prolongs tech debt around transient identity.

## Acceptance Criteria

- [ ] Public core commands, events, DTOs, and rustdoc no longer expose NodeId or NodeId compatibility variants.
- [ ] Actor internals, command routing, caches, watches, previews, metadata, search, and operations no longer accept NodeId as an addressing input or perform provider work from NodeId-only input.
- [ ] NodeId type definitions, constructors, hashing helpers, compatibility translators, and related exports are removed unless a remaining test proves they are dead-code cleanup outside this task.
- [ ] Tests and API snapshots prove removed NodeId command routes are absent, not retained as structured errors or deprecated compatibility routes.
- [ ] LocationRef command routes continue to cover navigation, scan, preview, metadata, search, watch, and operations without a compatibility fallback.
- [ ] The task resolution explicitly supersedes API-002 compatibility intent and the obsolete API-003 removal-or-rejection task so future work does not reintroduce a staged NodeId migration.
