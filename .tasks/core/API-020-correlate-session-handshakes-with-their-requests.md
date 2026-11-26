---
id: "API-020"
title: "Correlate session handshakes with their requests"
status: Done
priority: "Medium"
type: "Bug"
milestone: "0.3.1"
rules: ["SESSION-BOUNDARY"]
risk: "Medium"
impact: "Changes the public Handshake command, SessionCreated event, and WireCommand shape."
tags: ["api", "sessions", "events", "bug", "ready-for-agent"]
last_updated: 2026-11-26
---

## Summary

Command::Handshake carries no RequestId, and Event::SessionCreated carries only the new SessionId on the shared event stream. When two clients or windows handshake at the same time, neither can tell which SessionCreated belongs to it. Add request correlation to the handshake command, its wire form, and the created event so a client binds its Session by request identity. Separate Session event streams stay with REL-011.

## Acceptance Criteria

- [x] Handshake carries a RequestId in Command and WireCommand, and SessionCreated reports that RequestId with the new SessionId.
- [x] A public-interface test sends two concurrent handshakes and binds each created Session to its own request.
- [x] Command::request_id and the router dispatch trace report the handshake request.
- [x] Supported library, example, benchmark, and test targets compile against the new shape; retained desktop handshake call sites are migrated without expanding into the legacy application rewrite. cargo fmt --check, cargo check --workspace --exclude filer-app, and cargo test -p filer-core pass; isolated app validation targets are checked when UI-012 supplies them, and the existing executable exception is recorded.

## Verification

The wire regression failed before the fix because `Command::request_id()`
returned `None`. It checks that the `handshake` payload requires and preserves
the request ID. The public runtime test binds two concurrent handshakes by
request ID, and the dispatch trace test checks that the handshake records the
same ID.

`cargo fmt --check`, `cargo check --workspace --exclude filer-app`, the same
workspace check with `--all-targets`, and `cargo test -p filer-core` pass on
Linux. The isolated benchmark workspace passes its locked all-target check and
test suite.

The retained desktop client sends a request ID and matches it on
`SessionCreated`. CI excludes the legacy `filer-app` executable because it
depends on removed Core APIs such as `NodeId` and `FileNode`. UI-012
owns the isolated validation targets. [CI configuration](../../.github/workflows/ci.yml)
