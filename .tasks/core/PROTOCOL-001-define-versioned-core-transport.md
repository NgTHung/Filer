---
id: PROTOCOL-001
title: Define versioned core transport
status: To Do
priority: Medium
type: Epic
depends_on: ["core:API-001"]
rules: [CORE-LIBRARY, WIRE-SAFE-EXTENSIONS, SESSION-BOUNDARY]
risk: High
impact: "Defines serialization and server transport for non-desktop clients."
tags: [protocol, serde, server]
last_updated: "2026-10-02"
---

## Summary

Windows users need local file management and optional connections to a separate Linux machine. Stage a shared versioned protocol and thin Linux host around existing public core behavior. Core owns command meaning and execution; protocol types own encoding and compatibility; the host owns authentication, connection lifetime, and event routing; clients own presentation.

PROTOCOL-002 through PROTOCOL-006 deliver built-in read-only browsing after the Linux TUI proves the local interface. This milestone-free follow-on program adds no 0.3.1 release gate. Its initial delivery does not depend on MODULES-001, preview transport, WASM, TypeScript, or a WebSocket transport. The shared protocol crate and framing decision are recorded by PROTOCOL-002 before implementation.

PROTOCOL-007 owns remote mutation lifetime and recovery design. PROTOCOL-008 retains metadata/preview and extension transport follow-ons. REL-010 gates restricted access, and REL-011 gates independent clients sharing a runtime; both remain Deferred. The first browsing host uses one core process per authenticated connection with the OS user's permissions, one event consumer, and explicit bounded slow-client cleanup.

## Exit Criteria

- [ ] PROTOCOL-002 through PROTOCOL-006 deliver a versioned, correctness-tested browsing protocol, shared wire types, a thin Linux host, a remote adapter, and fault/latency evidence.
- [ ] Built-in transport works independently of the semantic extension plane; encoding preserves Location identity, correlation, structured errors, and client-relevant entry data.
- [ ] PROTOCOL-007 records remote mutation acceptance, disconnect uncertainty, status/recovery, retention limits, and staged implementation tasks before mutations are enabled remotely.
- [ ] PROTOCOL-008 records the remaining preview, metadata, semantic-output, and future binding work without widening the first browsing protocol.
- [ ] Restricted access and independent shared-runtime delivery remain gated on REL-010 and REL-011; native OS permissions and per-connection process isolation are described without promising either deferred capability.
