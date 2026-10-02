---
id: "PROTOCOL-002"
title: "Specify the built-in remote browsing contract"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:PROTOCOL-001"
depends_on: ["tui:TUI-005"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["protocol", "contracts", "remote", "enhancement", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

A Windows client must interpret Linux browsing results without interpreting Linux paths as Windows paths. Specify a minimal versioned read-only contract using the local TUI experience and existing WireCommand as evidence. Decide shared protocol-crate placement, dependency direction, encoding/framing, compatibility, and limits before writing transport code.

## Acceptance Criteria

- [ ] The contract enumerates handshake, navigation/history, directory pages/cursors, pipeline changes, refresh/watch, cancellation, basic metadata, structured errors, and close outcomes; unsupported commands reject explicitly.
- [ ] Host identity scopes Locations, Sessions, Requests, and cursors. Linux byte filenames and Windows native filenames have lossless identity encoding distinct from display text, tested in a cross-platform example matrix.
- [ ] Version negotiation, unknown messages/fields, malformed input, message/page limits, correlation, stale results, overflow, and read-only reconnect with a fresh Session have explicit observable rules.
- [ ] The first host uses one core process per connection under the authenticated OS user, with SSH-secured transport; connection ownership prevents arbitrary Session targeting and slow-client cleanup is bounded.
- [ ] A contract/ADR records ownership, shared crate dependencies, the chosen framing, and separate engine/network/decode/render measurements; subsequent implementation tickets are refined and marked ready only after the contract is reviewed.
