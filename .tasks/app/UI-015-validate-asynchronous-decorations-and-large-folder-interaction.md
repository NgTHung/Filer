---
id: "UI-015"
title: "Validate asynchronous decorations and large-folder interaction"
status: "To Do"
priority: "High"
type: "Feature"
parent: "app:UI-011"
milestone: "0.3.1"
depends_on: ["app:UI-014", "core:MODULES-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["testing", "performance", "validation", "enhancement", "ready-for-agent"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Add the existing GitDecorationsModule through its public extension command and semantic events. Request only visible locations within the advertised limit, invalidate badges by session/request/location, and keep directory data usable while Git runs or fails. Then record the 10,000-entry real-window proof using the CORE-028 fixture conventions and baseline metadata where reusable. Decoration integration and measurement evidence land separately.

## Acceptance Criteria

- [ ] A controlled slow Git backend proves listing and interaction continue before decoration completion; failures preserve the directory view and expose useful status.
- [ ] Only current visible-row identities receive badges; stale decoration batches after scrolling, navigation, or refresh are rejected and invalidations are coalesced.
- [ ] Automated tests cover late/stale/failing decoration events alongside paging, and a real-window run demonstrates badges arriving after an already usable listing.
- [ ] A reproducible smoke procedure records machine, OS/display, toolchain, revision, fixture, first-visible-page latency, input/frame samples, and decoration-on/off behavior.
- [ ] Every reproduced core failure links a concrete core regression task; GUI observations remain separate from CORE-032 virtual-view timing and UI-001 framework rankings.
