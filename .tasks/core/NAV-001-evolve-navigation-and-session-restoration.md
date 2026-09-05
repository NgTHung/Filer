---
id: "NAV-001"
title: "Evolve navigation and session restoration"
status: "To Do"
priority: "Medium"
type: "Epic"
milestone: "0.5.0"
rules: ["SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
impact: "Extends session state used by tabs, panes, workspaces, and transport."
tags: ["navigation", "sessions", "workspace", "enhancement", "needs-triage"]
last_updated: "2026-09-05"
---

## Summary

Provide frontend-independent restorable session state on top of the existing Location-native navigation history.

Back/forward navigation, history index, and can_forward already exist. NAV-002 first specifies restoration of one session's Location, history, and PipelineConfig, then creates implementation stages. Selection hints and provider recovery follow that contract; clients and ecosystem profiles own durable workspace layout. Future transport compatibility means a serializable snapshot contract here, with transport implementation owned by PROTOCOL-001.

## Exit Criteria

- [x] Core exposes first-class forward navigation (navigator Forward / can_forward / history index).
- [ ] Session snapshots restore location, history, pipeline configuration, selection hints, and active providers.
- [ ] Tabs and split panes can use independent sessions without shared mutable navigation state.
- [ ] Core snapshot references let clients restore their panes and provider profiles without storing client layout or provider secrets in core.
- [ ] Session snapshots have a serializable handoff contract that future PROTOCOL-001 transport can consume.
