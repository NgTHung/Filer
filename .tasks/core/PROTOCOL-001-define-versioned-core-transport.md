---
id: PROTOCOL-001
title: Define versioned core transport
status: To Do
priority: Medium
type: Epic
depends_on: [API-001, MODULES-001]
rules: [CORE-LIBRARY, WIRE-SAFE-EXTENSIONS, SESSION-BOUNDARY]
risk: High
impact: "Defines serialization and server transport for non-desktop clients."
tags: [protocol, serde, server]
last_updated: 2026-09-05
---

## Summary

Serialize public core contracts behind a versioned envelope and thin server transport.

ADR-0001 keeps current extensions compiled in and typed at their caller
interface. Portable identifiers, decoding, and version checks still belong to
this future transport. REL-010 and REL-011 retain the explicitly deferred
authorization and Session-stream work. Current native execution does not prove
restricted access, and competing receiver clones do not provide client isolation.

## Exit Criteria

- [ ] Public command, event, node, session, metadata, preview, operation, and pipeline types support serde.
- [ ] Commands and events use a versioned transport envelope.
- [ ] Unknown-field and forward-compatibility behavior has tests.
- [ ] filer-server remains a thin transport crate that depends on core.
- [ ] WebSocket lifecycle covers connection, session creation, event streaming, and destruction.
- [ ] Accessibility-relevant metadata survives transport.
- [ ] Future WASM or TypeScript bindings use the same protocol.
- [ ] Restricted-client support is gated on REL-010 enforcement, and claims of independent Session delivery are gated on REL-011; both remain Deferred until explicitly reactivated.
