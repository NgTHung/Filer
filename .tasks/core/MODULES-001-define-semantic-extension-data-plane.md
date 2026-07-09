---
id: MODULES-001
title: Define semantic extension data plane
status: Deferred
priority: High
type: Epic
parent: CORE-001
milestone: "0.4.0"
depends_on: [API-001, REL-001, MODULES-002]
rules: [WIRE-SAFE-EXTENSIONS, SEMANTIC-EXTENSION-OUTPUT, SESSION-BOUNDARY]
risk: High
impact: "Defines extension output consumed by desktop and future transport clients."
tags: [extensions, events, semantic-output]
last_updated: 2026-07-09
---

## Summary

Epic for the wire-safe semantic extension data plane. The work is decomposed into staged children: MODULES-003 (envelope schema), MODULES-004 (scoped subscriptions), MODULES-005 (trusted in-process host), and MODULES-006 (git decoration vertical slice as the exit proof). Preview envelope alignment stays in PREVIEW-001.

Parent remains CORE-001 for hierarchy only. This epic is not a CORE-001 or MILESTONE-003 exit gate. Candidate membership is draft milestone 0.4.0 (MILESTONE-005).

## Acceptance Criteria

- [ ] MODULES-003, MODULES-004, and MODULES-005 are Done.
- [ ] MODULES-006 proves the plane end to end: git decorations flow through envelopes with no listing-latency regression against the CORE-028 baseline.
- [ ] Extension output stays limited to client-neutral file-manager semantics across all children.
- [ ] The bridge depends on filer-ecosystem only when live host contracts require it.

## Rationale

Premature: filer-ecosystem has zero consumers workspace-wide and the data plane should be designed against a real consumer (MODULES-002 git decorations) rather than as a speculative wire contract. Defer until extensions are a near-term need.

MODULES-002 is the explicit dependency: it proves the in-process semantic contract first, and this epic generalizes that contract into wire-safe envelopes. The 0.3.0 decoration exit criterion is satisfied by MODULES-002 alone. This epic and its children stay Deferred until 0.4.0 work is intentionally started; reactivate them together when MILESTONE-005 begins.
