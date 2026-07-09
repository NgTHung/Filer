---
id: NAV-001
title: Evolve navigation and session restoration
status: To Do
priority: Medium
type: Epic
milestone: "0.5.0"
rules: [SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: Medium
impact: "Extends session state used by tabs, panes, workspaces, and transport."
tags: [navigation, sessions, workspace]
last_updated: 2026-07-09
---

## Summary

Provide frontend-independent restorable session state on top of the existing Location-native navigation history.

Back/forward navigation, history index, and can_forward already exist in the navigator after CORE-025. This epic focuses on snapshots, restore, multi-session workspace layout, and future transport handoff.

## Exit Criteria

- [x] Core exposes first-class forward navigation (navigator Forward / can_forward / history index).
- [ ] Session snapshots restore location, history, pipeline configuration, selection hints, and active providers.
- [ ] Tabs and split panes can use independent sessions without shared mutable navigation state.
- [ ] Workspace restore represents panes, locations, provider profiles, and frontend-owned layout references.
- [ ] Session handoff can cross a future server transport.
