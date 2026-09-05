---
id: "UI-014"
title: "Render a virtualized one-window validation browser"
status: "To Do"
priority: "High"
type: "Feature"
parent: "app:UI-011"
milestone: "0.3.1"
depends_on: ["app:UI-013"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["testing", "performance", "validation", "enhancement", "ready-for-agent"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Attach a provisional Iced renderer to UI-013 using the dependency already declared in filer-app. Build one window with a location entry, navigation controls, a directory viewport, focused/selected row feedback, and loading/error/retry status. Framework types stay in the adapter. Document one explicit launch command and keep this entry point independent of legacy screens. Land the window/wakeup integration before viewport rendering.

## Acceptance Criteria

- [ ] The documented validation command opens a real local folder; keyboard and pointer input can focus/select loaded rows, enter a folder, navigate back/up, and refresh.
- [ ] Only visible rows plus bounded overscan are constructed/formatted; tests or counters prove work stays proportional to the viewport with 10,000 loaded rows.
- [ ] Approaching the loaded boundary requests at most one next page; repaint alone never issues another request or clones the full row collection.
- [ ] Core events wake the window without continuous idle repaint; input remains usable during page arrival and a controllably slow provider request.
- [ ] Controls have accessible names and visible keyboard focus; automated state/viewport tests and a real-window smoke record cover loading, empty, failed, and refreshed views.
