---
id: "UI-013"
title: "Connect validation browsing to public core commands"
status: "To Do"
priority: "High"
type: "Feature"
parent: "app:UI-011"
milestone: "0.3.1"
depends_on: ["app:UI-012", "core:PIPELINE-003", "core:CORE-020"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["testing", "performance", "validation", "enhancement", "ready-for-agent"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Connect UI-012 state to a real FilerCore session through public commands and events. Support opening a Location, entering a folder, back/forward/up, refresh, and one pending continuation request. Keep event delivery bounded and event-driven. Pin command construction and event correlation with fake-port tests, then exercise a real temporary-directory provider through public core. Session lifecycle and navigation/paging integration land separately.

Use one runtime event consumer, as recorded in ADR-0001; receiver clones compete
for messages. This read-only track does not implement mutation draining or exit
recovery UI, which belong to REL-008 and deferred app:UI-016. Adapt to the
startup-only composition interface when API-018 lands.

## Acceptance Criteria

- [ ] A real public-core integration test opens a folder, appends a continuation, navigates away, and rejects events belonging to the old request or session.
- [ ] Input reduction performs no I/O; a bounded event bridge preserves terminal/error events and exposes an event-loop wakeup without idle polling.
- [ ] Duplicate continuation requests are suppressed, consumed cursors are not replayed, and invalid or expired cursors recover through a fresh request.
- [ ] Loading, empty, error, cancelled, and refreshing states remain distinct; a failed refresh retains the previous valid rows and supports retry.
- [ ] Read-only Session teardown cancels owned reads and reports channel/shutdown failures; focused tests verify quiescence using barriers instead of fixed sleeps without claiming accepted-mutation draining.
