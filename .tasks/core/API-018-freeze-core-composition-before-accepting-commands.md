---
id: "API-018"
title: "Freeze core composition before accepting commands"
status: "To Do"
priority: "High"
type: "Refactor"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["api", "architecture", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Implement the startup-only composition decision in ADR-0001. FilerCore currently starts routing in new() and permits load/on replacement while commands run. First introduce and test construction versus running states, then migrate built-ins and callers in reviewable commits. Preserve configurable startup composition and the default local setup; do not add live actor replacement.

## Acceptance Criteria

- [ ] A public setup path registers modules before command admission; the running handle cannot add or replace handlers.
- [ ] Duplicate registrations and missing required built-in wiring are reported during setup rather than leaving partially replaced routes; optional modules remain optional.
- [ ] Built-in command dispatch retains typed command identity without requiring caller-authored string keys.
- [ ] Test-first setup and public-client cases cover default/custom composition, rejected runtime registration, and startup failures; migrate examples and callers without claiming compatibility with removed runtime replacement.
- [ ] Implementation is split into reviewable stages; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
