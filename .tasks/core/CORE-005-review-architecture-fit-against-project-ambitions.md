---
id: CORE-005
title: Review architecture fit against project ambitions
status: Done
priority: High
type: Design
parent: CORE-004
rules: [CORE-LIBRARY, CORE-MECHANICS-BUILTIN, WIRE-SAFE-EXTENSIONS, SEMANTIC-EXTENSION-OUTPUT]
risk: High
impact: "Frames the central verdict consumed by every downstream review and remediation decision."
tags: [core, audit, architecture]
last_updated: 2026-06-13
---

## Summary

Assess whether the Location addressing model, actor/cancellation model, the planned extension data plane, the versioned transport, and cross-client reuse can carry the stated ambitions, or where the design risks collapse. Reference PROVIDER-001, MODULES-001, PROTOCOL-001 rather than restating them.

## Acceptance Criteria

- [x] Report at docs/reviews/filer-core/architecture-fit.md maps each ambition to the structures meant to support it.
- [x] Structural risks are severity-ranked with the conditions that would trigger a rewrite.
- [x] Follow-up task candidates are listed for each accepted structural risk.
