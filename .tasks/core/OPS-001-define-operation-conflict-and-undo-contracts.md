---
id: OPS-001
title: Define operation conflict and undo contracts
status: Done
priority: High
type: Design
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, CORE-MECHANICS-BUILTIN]
risk: High
impact: "Constrains future copy, move, conflict, and undo workflows."
tags: [operations, conflict, undo]
last_updated: 2026-06-24
---

## Summary

Define provider-aware conflict decisions and reversible-operation metadata before expanding operation workflows.

## Acceptance Criteria

- [x] Copy and move conflicts have structured resolution choices.
- [x] Reversible operations define the metadata needed for future undo.
- [x] Contracts describe provider-specific atomic and best-effort guarantees.
- [x] Design tests cover serialization and forward-compatible enum handling.
