---
id: PROVIDER-003
title: Plan provider sync and backup
status: Deferred
priority: Low
type: Epic
depends_on: [PROVIDER-002, OPS-001]
rules: [PROVIDER-ACCESS]
risk: High
impact: "Defines cross-provider synchronization, conflicts, and backup planning."
tags: [provider, sync, backup]
last_updated: 2026-09-05
---

## Summary

Add provider-neutral sync and backup plans without mixing portable profiles with secrets.

## Exit Criteria

- [ ] Two-provider sync produces a deterministic operation plan.
- [ ] Sync conflicts use explicit provider-aware strategies.
- [ ] Incremental backup planning records stable source and destination identity.
- [ ] Future server transport can synchronize profile operations separately from file contents.

## Rationale

Sync and backup are explicitly later than 0.5.0. The maintainer approved keeping the current queue focused on filer-core 0.3.1 on 2026-09-05; choose a later milestone and refine implementation stages before reactivation.
