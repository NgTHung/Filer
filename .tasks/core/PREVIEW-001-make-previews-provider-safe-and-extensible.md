---
id: PREVIEW-001
title: Make previews provider-safe and extensible
status: To Do
priority: Medium
type: Epic
milestone: "0.4.0"
rules: [PROVIDER-ACCESS, ACTOR-LONG-WORK, SEMANTIC-EXTENSION-OUTPUT]
risk: Medium
impact: "Changes preview, metadata, and cache contracts used by clients."
tags: [preview, metadata, provider]
last_updated: 2026-09-05
---

## Summary

Remove local-path assumptions and define stable lazy metadata and preview payloads on Location-native identity.

PREVIEW-002 specifies the provider-safe contracts after 0.3.1, then decomposes PREVIEW-003 into implementation stages. PREVIEW-003 owns built-in payload and cache access independently of extensions. PREVIEW-004 owns manifest registration and semantic events, with explicit dependencies on provider-safe access, envelopes, subscriptions, and the trusted host.

## Exit Criteria

### Provider-safe core (PREVIEW-002 and PREVIEW-003)

- [ ] Magic-byte reads and cache generation use provider-backed access.
- [ ] NodeEntry / Location-native rows document synchronous fields and lazy metadata guarantees.
- [ ] Nodes, metadata, and previews provide accessible labels without extra filesystem calls.
- [ ] Text, code, video, audio, markdown, document, hex, and binary payloads remain renderer-neutral.
- [ ] Thumbnail disk cache keys use stable identity or content hashes.

### Extension host (PREVIEW-004)

- [ ] Manifest-declared preview and metadata providers register through the core host once MODULES-001 exists.
- [ ] Extension preview and metadata status uses structured core events once MODULES-001 exists.
