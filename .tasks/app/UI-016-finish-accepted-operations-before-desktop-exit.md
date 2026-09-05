---
id: "UI-016"
title: "Finish accepted operations before desktop exit"
status: "Deferred"
priority: "High"
type: "Feature"
milestone: "0.5.0"
depends_on: ["core:REL-008", "app:UI-010"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["operations", "sessions", "errors", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Implement the desktop exit behavior accepted in ADR-0001 after Core supplies graceful completion and application work is reactivated. Keep this separate from the read-only UI-011 validation browser. The last window offers finish operations and quit or explicit cancellation; process-level operation state and event consumption outlive closed views.

## Acceptance Criteria

- [ ] Accepted active and queued mutations survive tab/window closure while their Session routing and operation outcomes remain available.
- [ ] Finish-and-quit keeps the process, Core, and event consumer alive until accepted operations and cleanup complete; cancellation is explicit and never implied by a timeout.
- [ ] Failure interrupts exit and retains or reopens an operations window with partial outcomes and retry, continue, and cancel-remainder controls.
- [ ] Controller/bridge tests cover closure during queued work and failure recovery, and a real-window smoke run verifies the exit behavior without moving UI state into Core.

## Rationale

The full application remains deferred. The maintainer approved this exit contract on 2026-09-05, but UI-011 stays limited to read-only validation; implement after explicit application-scope reactivation.
