---
id: "OPS-004"
title: "Admit mutations to bounded per-session FIFO queues"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:CORE-035", "core:REL-007"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["operations", "queue", "cancellation", "bug", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Replace the single per-Session mutation cancellation slot with bounded FIFO admission from ADR-0001. Starting create-folder during a copy currently cancels that copy. Introduce queue-state tests first, then integrate the extracted operation handlers and correlated public acceptance/rejection results in separate commits. Document the capacity accounting and chosen default; do not add concurrent mutations within a Session or durable storage.

## Acceptance Criteria

- [ ] Two accepted mutations in one Session execute in admission order; submitting the second never cancels the first, and different Sessions retain independent queues.
- [ ] Admission validates the command and reserves bounded capacity with an operation identity; full queues return an explicit busy rejection without starting or retaining rejected work.
- [ ] Public tests distinguish accepted, rejected, queued, running, and terminal outcomes; execution rechecks state that can change while a command waits.
- [ ] Individual queued and running operations remain explicitly cancellable, and cancellation, recovery, and closure controls remain serviceable when admission is full.
- [ ] Barrier-based tests cover copy followed by create, FIFO order, capacity exhaustion/recovery, cancellation, and cross-Session progress; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
