---
id: "API-020"
title: "Correlate session handshakes with their requests"
status: "To Do"
priority: "Medium"
type: "Bug"
milestone: "0.3.1"
rules: ["SESSION-BOUNDARY"]
risk: "Medium"
impact: "Changes the public Handshake command, SessionCreated event, and WireCommand shape."
tags: ["api", "sessions", "events", "bug", "ready-for-agent"]
last_updated: "2026-09-24"
---

## Summary

Command::Handshake carries no RequestId, and Event::SessionCreated carries only the new SessionId on the shared event stream. When two clients or windows handshake at the same time, neither can tell which SessionCreated belongs to it. Add request correlation to the handshake command, its wire form, and the created event so a client binds its Session by request identity. Separate Session event streams stay with REL-011.

## Acceptance Criteria

- [ ] Handshake carries a RequestId in Command and WireCommand, and SessionCreated reports that RequestId with the new SessionId.
- [ ] A public-interface test sends two concurrent handshakes and binds each created Session to its own request.
- [ ] Command::request_id and the router dispatch trace report the handshake request.
- [ ] filer-app, benches, and tests compile against the new shape; cargo fmt --check, cargo check --workspace, and cargo test -p filer-core pass.
