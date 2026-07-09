---
id: MODULES-006
title: Migrate git decorations onto the envelope data plane
status: Deferred
priority: High
type: Refactor
parent: MODULES-001
milestone: "0.4.0"
depends_on: [MODULES-003, MODULES-004, MODULES-005]
rules: [SEMANTIC-EXTENSION-OUTPUT]
risk: Medium
impact: "Proves the data plane end to end by porting the only real consumer onto it."
tags: [extensions, git, semantic-output]
last_updated: 2026-07-09
---

## Summary

Port the MODULES-002 in-process git decoration prototype onto the wire-safe envelope data plane as the proving vertical slice. The prototype's direct in-process contract is retired once the envelope path carries the same decorations with no regression in listing latency. This task is the 0.4.0 exit proof: if the envelope plane cannot carry git decorations cleanly, the schema work above is not done.

## Rationale

Staged decomposition of MODULES-001; reactivate when MILESTONE-005 (0.4.0) work begins.

