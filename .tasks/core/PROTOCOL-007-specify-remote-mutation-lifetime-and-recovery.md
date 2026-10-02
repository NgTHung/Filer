---
id: "PROTOCOL-007"
title: "Specify remote mutation lifetime and recovery"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-006", "core:REL-008", "core:OPS-006"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "operations", "sessions", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

A lost reply does not establish whether a remote file operation was accepted or completed. Define remote mutation admission and recoverable status after the browsing connection is proven and local mutation guarantees land. The per-connection browsing process is not evidence that operations survive disconnect.

## Acceptance Criteria

- [ ] The contract distinguishes submission, rejection, acceptance, and terminal results; duplicate requests or uncertain delivery never automatically replay destructive operations.
- [ ] An observable policy states whether accepted work continues after disconnect, how the owning user queries its status and recovers, which bounded outcomes remain retained, and when closure releases state.
- [ ] Process-crash survival, connection loss, explicit cancellation, and partial failure have separate guarantees; host process lifetime is sufficient for the chosen disconnect guarantee.
- [ ] The design maps to OPS-004/005 and REL-008, gates shared-runtime isolation and restricted clients on REL-011/010, and creates reviewable host/adapter/GUI implementation stages with fault-test criteria.
