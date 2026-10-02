---
id: "PROTOCOL-006"
title: "Verify remote browsing faults and end-to-end costs"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-005"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "remote", "performance", "testing", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

A local round trip cannot establish remote correctness or overhead. Exercise the host and adapter under slow delivery, dropped connections, invalid messages, and a real Windows-to-Linux connection. Record each timing layer separately before choosing further encoding or transport optimizations.

## Acceptance Criteria

- [ ] Deterministic fault tests cover partial frames, bounded oversized messages, stalled consumers, cancelled requests, stale pages, incompatible versions, and disconnect cleanup without unbounded buffering.
- [ ] A 10,000-entry fixture produces correct ordered pages and responsive client input under recorded network conditions; unsorted streaming and globally sorted listing costs are reported separately.
- [ ] A recorded Windows-to-Linux smoke run and reproducible command report engine, server core, transport, client decode, and visible-view costs, payload bytes, exchange counts, and memory limits.
- [ ] Results make no shared-runtime multi-client isolation or mutation recovery claim; failures have regression coverage and transport/client checks pass.
