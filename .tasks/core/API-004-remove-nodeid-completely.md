---
id: API-004
title: Remove NodeId completely
status: Done
priority: High
type: Epic
parent: CORE-001
milestone: "0.3.0"
depends_on: [CORE-025]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Removes NodeId from public and internal core contracts instead of preserving compatibility surfaces."
tags: [api, nodeid, location, compatibility]
last_updated: 2026-08-30
---

## Summary

Remove NodeId from filer-core contracts and implementation entirely. Do not preserve deprecated routes, compatibility event variants, or translation paths, because keeping them prolongs tech debt around transient identity.

A single-pass removal was attempted on 2026-07-08 and reverted: it deleted the test suite (844 tests) instead of migrating it, so its passing `cargo test` evidence was vacuous. This epic stages the removal so coverage survives every step: tests migrate first, then the public surface, then internals, then the type itself. Each child stays within the change-size guidance.

This epic supersedes the API-002 compatibility intent and the obsolete API-003 removal-or-rejection task so future work does not reintroduce a staged NodeId migration.

## Exit Criteria

- [x] API-005, API-006, API-007, and API-008 are Done.
- [x] Public core commands, events, DTOs, and rustdoc no longer expose NodeId or NodeId compatibility variants.
- [x] NodeId type definitions, constructors, hashing helpers, and compatibility translators are removed.
- [x] The full filer-core test suite passes at every stage with no net loss of coverage.
