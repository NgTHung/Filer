---
id: "UI-012"
title: "Isolate validation-client state and test targets"
status: "To Do"
priority: "High"
type: "Feature"
parent: "app:UI-011"
milestone: "0.3.1"
depends_on: ["core:API-004"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK", "PIPELINE-TRANSFORMS"]
risk: "Medium"
tags: ["testing", "performance", "validation", "enhancement", "ready-for-agent"]
whitepaper: "docs/architecture/filer-app.md"
last_updated: "2026-09-05"
---

## Summary

Create the smallest testable validation-client boundary inside filer-app. Isolate its build/test targets from legacy app modules that still import NodeId/FileNode, preserving the legacy source without migrating its unrelated features. Build framework-free directory state and intent reduction before adding a window. Reuse existing helpers where they match current contracts. Land target isolation first, then page/selection state in separate commits under repository diff guidance.

## Acceptance Criteria

- [ ] The validation library or equivalent test target compiles against current core without importing legacy app modules or Iced types into its model/controller.
- [ ] Tests written before implementation cover first-page replacement, continuation append, gap/overlap rejection, request/session mismatch, and refresh generations using NodeEntry identity.
- [ ] Loaded-row focus and selection use LocationRef identity and survive valid page append without cloning the entire directory or treating indices as identity.
- [ ] Directory state grows with loaded rows; row projection accepts a visible range and has no filesystem or rendering side effects.
- [ ] Documented target-specific check/test commands pass independently of the legacy executable; modules stay under 700 lines.
