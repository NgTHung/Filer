---
id: API-001
title: Canonicalize public command names
status: Done
priority: High
type: Refactor
parent: CORE-001
milestone: "0.3.0"
rules: [CORE-LIBRARY]
risk: High
impact: "Renames public commands used by current and future core clients."
tags: [api, compatibility, location]
last_updated: 2026-06-10
---

## Summary

Make Location-native commands canonical, label path or NodeId entry points as compatibility APIs, and define an unversioned serializable DTO for built-in commands. Versioned transport envelopes and extension payload serialization remain outside this task.

## Acceptance Criteria

- [x] Location-native commands occupy canonical public names.
- [x] Path and NodeId commands use explicit Compat names.
- [x] `WireCommand` serializes every built-in command with stable snake_case type labels and converts to and from runtime `Command`.
- [x] Runtime extension command conversion fails with a typed error until MODULES-001 defines its wire-safe payload.
- [x] Canonical and compatibility commands retain distinct stable dispatch keys.
- [x] Command-routing tests cover every canonical and compatibility command family.
- [x] Migration documentation lists old and new Rust names, DTO limits, and canonical Location-based usage.
