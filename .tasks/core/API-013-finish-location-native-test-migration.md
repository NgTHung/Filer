---
id: "API-013"
title: "Finish Location-native test migration"
status: Done
priority: "High"
type: "TestDebt"
parent: "core:API-005"
milestone: "0.3.0"
rules: ["CORE-LIBRARY"]
risk: "Medium"
impact: "Removes unmarked compatibility identity from ordinary preview and API handle tests so API-005 can close before NodeId removal."
tags: ["api", "nodeid", "location", "testing"]
last_updated: 2026-08-12
---

## Summary

Finish the API-005 audit remediation by converting ordinary previewer and FilerCore handle tests to Location-native commands and events. Keep provider-shaped FileNode fixtures and explicitly labeled API-006 or API-008 compatibility pins.

## Acceptance Criteria

- [x] Ordinary previewer behavior tests use PreviewEventMode::Location and native preview or metadata events; only explicit compatibility-route tests retain compatibility modes.
- [x] FilerCore session-validation tests use the LocationRef Navigate command; compatibility command tests remain isolated and labeled for API-006.
- [x] Every remaining NodeId, FileNode, or compatibility identity use in filer-core tests is provider-shaped setup or explicitly labeled for its downstream removal task.
- [x] The full filer-core suite and ignored stress suite pass with no reduction in test count.
