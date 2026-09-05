---
id: "OPS-005"
title: "Pause a session mutation queue after operation failure"
status: "To Do"
priority: "High"
type: "Feature"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:OPS-004"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["operations", "queue", "errors", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Implement the failure rule from ADR-0001 without assuming FIFO operations are independent. A failed copy must prevent a following source delete from starting automatically. Add failure/paused/recovery state tests first, then public recovery commands and correlated partial outcomes. Failed operations are settled; unattempted work stays accepted and paused until an explicit client decision.

## Acceptance Criteria

- [ ] A failed mutation reports its error and known partial changes, then pauses only the remaining mutation queue for its Session.
- [ ] Other Sessions and unrelated read requests continue; a failed-copy-then-delete regression proves the delete performs no filesystem work before recovery.
- [ ] The client can explicitly retry, continue the remaining queue, or cancel queued work; recovery never silently replays a partially completed mutation or promises rollback.
- [ ] Paused state and queued operation identities remain observable after the originating view closes; each cancelled or completed operation has a correlated terminal outcome.
- [ ] Tests cover each recovery choice, partial failure, repeated failure, and queue isolation; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
