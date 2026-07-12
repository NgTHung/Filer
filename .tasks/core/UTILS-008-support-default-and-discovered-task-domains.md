---
id: UTILS-008
title: Support default and discovered task domains
status: To Do
priority: High
type: Feature
parent: UTILS-005
depends_on: [UTILS-010]
risk: High
impact: "Changes task loading, creation, validation, and identity uniqueness for every project domain."
tags: [tooling, tasks, domains, namespaces]
last_updated: 2026-07-12
---

## Summary

Replace Filer-specific fixed domains with project domains governed by project configuration and the namespace contract. Simple projects may keep everything in an ordinary domain named `default`, while larger projects keep separate domain directories with overlapping local task IDs. Commands never select `default` implicitly.

## Acceptance Criteria

- [ ] `.tasks/default` is accepted as an ordinary task domain with no implicit resolution or creation behavior.
- [ ] `add` accepts the canonical domain-qualified task ID as shorthand for separate `--domain` and local `--id` arguments; the domain is always explicit, and conflicting inputs fail clearly.
- [ ] Task loading discovers valid project domains from project configuration instead of requiring the fixed core, app, and ecosystem list.
- [ ] Validation models identity as domain plus local ID and rejects duplicates only when both parts match.
- [ ] Two domains may contain the same local task ID and both tasks remain visible through list, filtering, sorting, show data, and validation.
- [ ] Domain and task creation reject invalid or reserved domain names with actionable errors.
- [ ] Existing core, app, ecosystem, and milestones directories remain readable through the compatibility contract.
- [ ] CLI help, examples, and task-tracking documentation describe qualified-ID creation, explicit domain requirements, and `default` as an ordinary domain without retaining fixed-domain guidance.
- [ ] Tests cover explicit creation in the `default` domain, qualified-ID creation, conflicting domain inputs, arbitrary project domains, duplicate local IDs across domains, same-domain duplicates, and reserved entries.
