---
id: MILESTONE-006
title: Power file manager and providers
status: To Do
priority: Medium
type: Milestone
milestone: "0.5.0"
depends_on: [MILESTONE-005]
risk: High
impact: "Grows advanced local file-manager workflows and first-class archive or remote provider surfaces."
tags: [power, providers, operations, draft]
last_updated: 2026-07-09
---

## Summary

Power file-manager workflows: next-page cost proportional to page size, professional operations, search roots, session restore, archives as folders, and at most one carefully chosen remote provider. Multi-client transport (PROTOCOL-001), sync/backup, WASM hosting, and marketplace stay later than 0.5.0.

Split expectation: this spans two themes, power-local (PIPELINE-002, OPS-002, SEARCH-001, NAV-001) and providers (archives, one remote provider). When this milestone becomes next in line, split it along that seam unless the combined scope has shrunk; power-local lands first because the provider half depends on its paging and search contracts.

## Draft policy

This milestone is a draft plan. You or any agent may modify it as much as needed (exit criteria, membership, priority, depends_on, title, or replacement by a better split) until work for 0.5.0 has started. Work has started when this milestone or any task with `milestone: "0.5.0"` first moves to `In Progress`. Until then, treat this file as editable intent, not a locked commitment. After work starts, change scope only deliberately and record why.

## Candidate membership

Power-local half:

- PIPELINE-002
- OPS-002
- SEARCH-001
- NAV-001

Provider half:

- Archive productization tasks when filed
- At most one remote provider productization task when chosen

Explicitly not this milestone: PROTOCOL-001, PROVIDER-003, marketplace, WASM sandbox.

## Exit Criteria

- [ ] PIPELINE-002 next-page work is proportional to page size, not full directory rewalk (audit F24), measured by extending the CORE-028 harness.
- [ ] OPS-002 advanced operations land on OPS-001 conflict and undo contracts.
- [ ] SEARCH-001 scoped roots and provider search delegation work without changing result contracts.
- [ ] NAV-001 session snapshots restore location, history, and pipeline config for multi-session clients.
- [ ] Archives are navigable as Location-segmented folders in product paths used by the app.
- [ ] At most one non-local provider path is productized only if contracts and paging remain honest; sync/backup and marketplace stay out of scope.
