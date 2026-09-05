---
id: "REL-011"
title: "Isolate session event delivery from stalled clients"
status: "Deferred"
priority: "High"
type: "Feature"
parent: "core:PROTOCOL-001"
milestone: "0.5.0"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["sessions", "events", "async", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Retain the deferred Session-stream branch of ADR-0001. Current Core event receivers compete for a shared stream, requiring one consumer to distribute events. Define bounded per-Session delivery and slow/disconnected-client behavior before promising independent client delivery. Preserve lifecycle and terminal outcomes without introducing an unbounded relay.

## Acceptance Criteria

- [ ] A refined interface delivers each client its Session events without competing receiver clones stealing another Session output.
- [ ] Tests show a stalled Session cannot prevent a healthy Session from receiving work and completion within the chosen delivery policy.
- [ ] Disconnect, queue overflow, terminal outcomes, and Session cleanup are explicit and bounded; clients migrate from the shared dispatcher deliberately.

## Rationale

The maintainer chose to defer separate Session streams on 2026-09-05. Shared backpressure and one distributing event consumer remain accepted limitations of the current runtime.
