---
id: "REL-010"
title: "Enforce provider-aware authorization before restricted client support"
status: "Deferred"
priority: "High"
type: "Feature"
parent: "core:PROTOCOL-001"
milestone: "0.5.0"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["sessions", "validation", "providers", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Retain the deferred authorization branch of ADR-0001. Current SessionPolicy takes local paths and is not enforced through production dispatch. Before claiming restricted-client support, define provider-aware action and Location checks that apply through every filesystem execution route. Core policy can narrow OS-granted rights; it cannot grant missing rights or require root.

## Acceptance Criteria

- [ ] A refined contract covers direct, queued, segmented, and extension-originated access and rechecks authority where queued execution can observe changed state.
- [ ] Public tests prove denied actions cannot reach the prohibited filesystem work and allowed native actions preserve the OS permission model.
- [ ] Restricted-client transport is gated on enforced authorization; input validation is not presented as authorization.

## Rationale

The maintainer deferred restricted-Session authorization on 2026-09-05. Keep this outside active 0.3.1 work and refine before any restricted-client implementation.
