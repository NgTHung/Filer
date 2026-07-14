---
id: UTILS-012
title: Support concurrent multi-project library access
status: Done
priority: Medium
type: Feature
parent: UTILS-005
depends_on: [UTILS-010]
risk: High
impact: "Adds write serialization and stale-state detection to task mutation for long-lived library consumers."
tags: [tooling, tasks, library, concurrency]
last_updated: 2026-07-14
---

## Summary

Long-lived consumers such as the planned web UI keep several projects open in one process and mutate tasks concurrently. Serialize writes per project and detect external on-disk changes so readers never observe partial or stale task state.

## Acceptance Criteria

- [x] Mutating operations on one project are serialized so concurrent callers cannot interleave file writes.
- [x] Task file writes are atomic on disk, and a failed mutation leaves no partially written task file.
- [x] Consumers can detect when on-disk task or configuration state changed after loading and reload cheaply.
- [x] Several projects opened in one process stay isolated: mutations in one never lock, reload, or alter another.
- [x] Public Rust documentation explains project-handle isolation, write serialization, atomic-write guarantees, stale-state detection, and consumer recovery behavior.
- [x] Tests cover concurrent mutations on one project, external modification detection, failed-write cleanup, and multi-project isolation.
