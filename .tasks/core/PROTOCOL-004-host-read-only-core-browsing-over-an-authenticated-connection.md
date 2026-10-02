---
id: "PROTOCOL-004"
title: "Host read-only core browsing over an authenticated connection"
status: "To Do"
priority: "Medium"
type: "Feature"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-003", "core:API-018", "core:REL-007", "core:REL-014"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "server", "sessions", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

A headless Linux process exposes core browsing to one authenticated Windows connection without taking over filesystem behavior. Implement the contract in a thin executable using one event consumer and SSH-secured transport under the authenticated OS user. Freeze core composition before admitting commands.

## Acceptance Criteria

- [ ] The host starts one core runtime per connection, binds the created Session to that connection, and rejects forged Session ownership, mutation messages, malformed values, and incompatible versions.
- [ ] A bounded reader/writer routes correlated commands and Session events without competing receiver clones or blocking sends; slow/disconnected clients release read work and resources under the specified policy.
- [ ] Provider operations and pipeline transformations remain in core; credentials stay outside core and the host neither requires root nor exposes an unauthenticated network listener.
- [ ] Subprocess tests cover handshake, browse/page/watch, protocol rejection, slow readers, disconnect cleanup, and graceful read-only shutdown; host/protocol/core checks pass.
