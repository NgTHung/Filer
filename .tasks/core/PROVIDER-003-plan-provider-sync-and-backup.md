---
id: PROVIDER-003
title: Plan provider sync and backup
status: To Do
priority: Low
type: Epic
depends_on: [PROVIDER-002, OPS-001]
rules: [PROVIDER-ACCESS]
risk: High
impact: "Defines cross-provider synchronization, conflicts, and backup planning."
tags: [provider, sync, backup]
last_updated: 2026-06-06
---

## Summary

Add provider-neutral sync and backup plans without mixing portable profiles with secrets.

## Exit Criteria

- [ ] Two-provider sync produces a deterministic operation plan.
- [ ] Sync conflicts use explicit provider-aware strategies.
- [ ] Incremental backup planning records stable source and destination identity.
- [ ] Future server transport can synchronize profile operations separately from file contents.
