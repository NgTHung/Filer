---
id: "PROTOCOL-008"
title: "Specify preview and semantic-output transport extensions"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:PROTOCOL-001"
depends_on: ["core:PROTOCOL-006", "core:MODULES-003", "core:PREVIEW-003"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "WIRE-SAFE-EXTENSIONS"]
risk: "Medium"
tags: ["protocol", "preview", "semantic-output", "enhancement", "needs-triage"]
last_updated: "2026-10-02"
---

## Summary

The initial browsing protocol cannot silently expand into preview streaming or type-erased extension transport. Define later payload and compatibility stages after provider-safe previews and semantic envelopes establish their own contracts. Preserve the broader transport ambitions without blocking built-in browsing.

## Acceptance Criteria

- [ ] The design maps provider-safe preview and extended metadata types plus MODULES-003 semantic envelopes into versioned messages with payload limits, cancellation, and accessibility-relevant labels.
- [ ] Preview/file content streaming does not block control messages or terminal outcomes; runtime extension Any payloads never cross the wire.
- [ ] Any WebSocket transport or WASM/TypeScript binding uses the same contract; separate implementation tasks identify needed consumers instead of making all bindings an initial gate.
