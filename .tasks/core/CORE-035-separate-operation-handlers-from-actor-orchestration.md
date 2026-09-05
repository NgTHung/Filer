---
id: "CORE-035"
title: "Separate operation handlers from actor orchestration"
status: "To Do"
priority: "Medium"
type: "Refactor"
parent: "core:CORE-019"
milestone: "0.3.1"
rules: ["ACTOR-LONG-WORK", "PROVIDER-ACCESS"]
risk: "Medium"
tags: ["core", "refactor", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Split modules/operations/operator.rs along existing command, transfer, mutation, and shared event/cache support responsibilities. Preserve public imports through re-exports. First move command and shared support code mechanically, then transfer handlers, then mutation handlers in separate commits. Each complex diff stays below 700 changed lines; document any larger mechanical move. Use the existing operator_test cases as the behavior contract.

Keep this extraction behavior-preserving. OPS-004 depends on it to replace the
current per-Session cancellation slot with bounded FIFO scheduling. OPS-005 and
REL-008 then add failure recovery and graceful closure under ADR-0001; do not mix
those behavior changes into this mechanical split.

## Acceptance Criteria

- [ ] Operation modules each stay under 700 lines and share event, provider-context, and cache plumbing without changing public command or event types.
- [ ] Copy, move, delete, rename, and create preserve operation identity, timeout, cancellation, cache invalidation, and terminal-event behavior.
- [ ] Existing operator and operation integration tests pass; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
