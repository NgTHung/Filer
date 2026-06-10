---
id: API-001
title: Canonicalize public command names
status: To Do
priority: High
type: Refactor
parent: CORE-001
milestone: "0.3.0"
rules: [CORE-LIBRARY]
risk: High
impact: "Renames public commands used by current and future core clients."
tags: [api, compatibility, location]
last_updated: 2026-06-06
---

## Summary

Make Location-native commands canonical and label path or NodeId entry points as compatibility APIs.

## Acceptance Criteria

- [ ] Location-native commands occupy canonical public names.
- [ ] Path and NodeId commands use explicit Compat names.
- [ ] Compatibility aliases and migration documentation prevent accidental ambiguit```y.
- [ ] Serialization and command-routing tests cover canonical and compatibility variants.
