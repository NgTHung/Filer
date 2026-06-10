---
id: OPS-001
title: Define operation conflict and undo contracts
status: To Do
priority: High
type: Design
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, CORE-MECHANICS-BUILTIN]
risk: High
impact: "Constrains future copy, move, conflict, and undo workflows."
tags: [operations, conflict, undo]
last_updated: 2026-06-06
---

## Summary

Define provider-aware conflict decisions and reversible-operation metadata before expanding operation workflows.

## Acceptance Criteria

- [ ] Copy and move conflicts have structured resolution choices.
- [ ] Reversible operations define the metadata needed for future undo.
- [ ] Contracts describe provider-specific atomic and best-effort guarantees.
- [ ] Design tests cover serialization and forward-compatible enum handling.
