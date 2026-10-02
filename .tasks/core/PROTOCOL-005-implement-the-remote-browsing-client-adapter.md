---
id: "PROTOCOL-005"
title: "Implement the remote browsing client adapter"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-004"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "remote", "api", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

Local Windows core and remote Linux core should exercise the same client browsing behavior. Implement a remote adapter over the reviewed wire contract; use the second adapter to extract shared client-facing intent/result handling where actual call sites demonstrate duplication.

## Acceptance Criteria

- [ ] The adapter supports the complete read-only contract through one persistent connection, with explicit host identity and ownership of request/session correlation.
- [ ] Local and remote adapters pass the same browsing behavior suite for paging, ordering, stale results, cancellation, errors, refresh, and basic metadata.
- [ ] Linux Locations remain opaque to Windows filesystem code; metadata and updates are batched so rendering a directory does not require a network exchange per row.
- [ ] Connection errors remain observable; reconnect creates a fresh Session and invalidates old cursors and messages. Adapter/protocol integration tests pass.
