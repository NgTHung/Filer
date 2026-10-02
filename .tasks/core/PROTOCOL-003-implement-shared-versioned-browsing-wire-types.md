---
id: "PROTOCOL-003"
title: "Implement shared versioned browsing wire types"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-002"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "serde", "compatibility", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

The local runtime types are not a complete portable wire contract: WireCommand is unversioned and Event lacks serialization. Implement the PROTOCOL-002 built-in subset in the shared protocol crate, with explicit conversion at the core adapter. Keep UI frameworks, sockets, provider execution, and type-erased extension payloads out of the wire module.

## Acceptance Criteria

- [ ] Command/event envelopes, version negotiation, and conversion support every specified browsing message and reject unsupported or incompatible messages with correlated protocol failures.
- [ ] Round-trip and compatibility fixtures cover unknown fields/types, invalid versions, configured limits, errors, page state, and lossless native-name identity across Linux and Windows.
- [ ] Runtime and wire data have documented ownership and avoid duplicated transformation logic; encoding cost, page size, and allocations are measured on the agreed fixtures.
- [ ] Protocol and core checks/tests pass; no filesystem or network work occurs in encoding/decoding.
