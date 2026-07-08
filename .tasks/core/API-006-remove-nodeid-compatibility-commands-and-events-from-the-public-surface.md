---
id: API-006
title: Remove NodeId compatibility commands and events from the public surface
status: To Do
priority: High
type: Refactor
parent: API-004
milestone: "0.3.0"
depends_on: [API-005]
rules: [CORE-LIBRARY]
risk: High
impact: "Deletes the compatibility command routes and event variants so LocationRef is the only public addressing contract."
tags: [api, nodeid, location]
last_updated: 2026-07-08
---

## Summary

Delete the NodeId and path compatibility command variants, wire-command routes, and compatibility event variants from commands.rs, wire_commands.rs, and events.rs. Removed routes must be absent, not retained as structured errors or deprecated stubs. Convert the compatibility pins marked by API-005 into absence tests proving the routes are gone from the wire surface.

## Acceptance Criteria

- [ ] Public commands, wire commands, and events expose no NodeId or path-compatibility variants.
- [ ] Absence tests prove removed routes fail to deserialize rather than route to structured errors.
- [ ] LocationRef routes cover navigation, scan, preview, metadata, search, watch, and operations without a compatibility fallback.
- [ ] The full filer-core suite passes.
