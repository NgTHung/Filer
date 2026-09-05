---
id: MODULES-001
title: Define semantic extension data plane
status: Deferred
priority: High
type: Epic
milestone: "0.4.0"
depends_on: [API-001, REL-001, MODULES-002]
rules: [WIRE-SAFE-EXTENSIONS, SEMANTIC-EXTENSION-OUTPUT, SESSION-BOUNDARY]
risk: High
impact: "Defines extension output consumed by desktop and future transport clients."
tags: [extensions, events, semantic-output]
last_updated: 2026-09-05
---

## Summary

Epic for the wire-safe semantic extension data plane. The work is decomposed into staged children: MODULES-003 (envelope schema), MODULES-004 (scoped subscriptions), MODULES-005 (trusted in-process host), and MODULES-006 (git decoration vertical slice as the exit proof). Preview envelope alignment stays in PREVIEW-001.

This epic belongs to 0.4.0 independently of the completed CORE-001 hierarchy. On 2026-09-05, its historical parent was removed so reactivation can make its children executable. MODULES-003 carries the explicit prior-milestone gate. Preview host integration is tracked by PREVIEW-004.

## Exit Criteria

- [ ] MODULES-003, MODULES-004, and MODULES-005 are Done.
- [ ] MODULES-006 proves the plane end to end: git decorations flow through envelopes with no listing-latency regression against the CORE-028 baseline.
- [ ] Extension output stays limited to client-neutral file-manager semantics across all children.
- [ ] The bridge depends on filer-ecosystem only when live host contracts require it.

## Rationale

Keep the extension data plane deferred while 0.3.1 local excellence is active. The completed MODULES-002 prototype supplies the concrete semantic consumer for the later envelope design.

MODULES-002 is the explicit dependency: it proves the in-process semantic contract first, and this epic generalizes that contract into wire-safe envelopes. The 0.3.0 decoration exit criterion is satisfied by MODULES-002 alone. This epic and its children stay Deferred until 0.4.0 work is intentionally started; reactivate them together when MILESTONE-005 begins.
