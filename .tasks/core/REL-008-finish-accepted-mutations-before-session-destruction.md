---
id: "REL-008"
title: "Finish accepted mutations before session destruction"
status: "To Do"
priority: "High"
type: "Feature"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:OPS-005", "core:REL-014"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["sessions", "operations", "async", "events", "enhancement", "ready-for-agent"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-10-02"
---

## Summary

Implement Session closure and graceful runtime completion from ADR-0001. Current destruction removes the Session and emits SessionDestroyed before cleanup hooks; runtime shutdown cancels tracked work. First add lifecycle and provider-work barriers, then integrate queue draining and the public completion contract. Audit underlying blocking filesystem work as well as tracked futures. Keep the event consumer active in all completion tests.

## Acceptance Criteria

- [ ] Closing rejects new work, cancels disposable reads, and continues every accepted queued or running mutation; recovery and explicit cancellation remain available while closing.
- [ ] Session identity, operation outcomes, and event routing remain usable until work settles; SessionDestroyed is emitted only after completion and resource cleanup, with no later Session work or events.
- [ ] A failed queue keeps closure pending and exposes recovery; no timeout silently discards accepted mutations or reports successful destruction.
- [ ] The public graceful runtime path drains accepted work before releasing actors and event delivery; explicit cancellation has a distinct outcome and process-crash survival is not promised.
- [ ] Barrier-based tests cover close during active/queued mutations, failure while closing, recovery, terminal ordering, and underlying filesystem quiescence; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.

- [ ] A real LocalFs regression occupies the only blocking worker, queues a copy behind it, requests shutdown, and proves shutdown cannot report completion before that queued native job settles. Releasing the worker after completed shutdown causes no later filesystem changes.
- [ ] Keep provider-work lifetime tests and queue-draining integration in separate commits; cancellation of an outer future is never treated as proof that native work stopped.
