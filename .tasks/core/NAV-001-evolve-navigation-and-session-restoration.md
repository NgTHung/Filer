---
id: NAV-001
title: Evolve navigation and session restoration
status: To Do
priority: Medium
type: Epic
rules: [SESSION-BOUNDARY, ACTOR-LONG-WORK]
risk: Medium
impact: "Extends session state used by tabs, panes, workspaces, and transport."
tags: [navigation, sessions, workspace]
last_updated: 2026-06-06
---

## Summary

Provide frontend-independent navigation history and restorable session state.

## Exit Criteria

- [ ] Core exposes first-class forward navigation.
- [ ] Session snapshots restore location, history, pipeline configuration, selection hints, and active providers.
- [ ] Tabs and split panes can use independent sessions without shared mutable navigation state.
- [ ] Workspace restore represents panes, locations, provider profiles, and frontend-owned layout references.
- [ ] Session handoff can cross a future server transport.
