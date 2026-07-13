---
id: UTILS-008
title: Support default and discovered task domains
status: Done
priority: High
type: Feature
parent: UTILS-005
depends_on: [UTILS-010]
risk: High
impact: "Changes task loading, creation, validation, and identity uniqueness for every project domain."
tags: [tooling, tasks, domains, namespaces]
last_updated: 2026-07-13
---

## Summary

Replace Filer-specific fixed domains with project domains governed by project configuration and the namespace contract. Simple projects may keep everything in an ordinary domain named `default`, while larger projects keep separate domain directories with overlapping local task IDs. Commands never select `default` implicitly.

## Acceptance Criteria

- [x] `.tasks/default` is accepted as an ordinary task domain with no implicit resolution or creation behavior.
- [x] `add` accepts the canonical domain-qualified task ID as shorthand for separate `--domain` and local `--id` arguments; the domain is always explicit, and conflicting inputs fail clearly.
- [x] Task loading discovers valid project domains from project configuration instead of requiring the fixed core, app, and ecosystem list.
- [x] Validation models identity as domain plus exact local-ID string, rejects duplicates only when both parts match, and keeps numeric spellings such as `WORK-001` and `WORK-1` distinct.
- [x] Two domains may contain the same local task ID and both tasks remain visible through list, filtering, sorting, show data, and validation.
- [x] Configuration, domain creation, and task creation reject invalid names and Windows device names such as `con`, `nul`, `com1`, and `lpt1` with actionable errors on every platform.
- [x] Existing core, app, ecosystem, and milestones directories remain readable through the compatibility contract.
- [x] CLI help, examples, and task-tracking documentation describe qualified-ID creation, explicit domain requirements, and `default` as an ordinary domain without retaining fixed-domain guidance.
- [x] Tests cover explicit creation in the `default` domain, qualified-ID creation, conflicting domain inputs, arbitrary project domains, exact-string local IDs, cross-domain and same-domain duplicates, reserved entries, and Windows device names.
