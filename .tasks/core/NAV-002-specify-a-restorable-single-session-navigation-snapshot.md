---
id: "NAV-002"
title: "Specify a restorable single-session navigation snapshot"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:NAV-001"
milestone: "0.5.0"
depends_on: ["milestones:MILESTONE-005"]
rules: ["SESSION-BOUNDARY", "CORE-LIBRARY"]
risk: "Medium"
tags: ["core", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Define the first NAV-001 slice: restore one session's Location, history, and PipelineConfig through public core contracts. Inspect navigation/state.rs and session lifecycle. Keep pane layout and durable workspace storage with their existing app/ecosystem owners; core accepts provider profile references and never persists secrets.

## Acceptance Criteria

- [ ] The design specifies snapshot/version validation, missing Location/provider recovery, ephemeral provider handling, and session isolation.
- [ ] A behavioral matrix covers successful restore, invalid history, unavailable providers, and independent sessions without requiring a transport implementation.
- [ ] Bounded implementation children cover the first slice; remaining selection hints and workspace/handoff integration stay mapped in NAV-001 with ownership and dependencies.
