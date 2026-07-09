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
last_updated: 2026-07-09
---

## Summary

Remove local-path assumptions and define stable lazy metadata and preview payloads on Location-native identity.

Provider-safe preview and metadata work does not wait on MODULES-001. Manifest registration and extension-emitted preview status require the wire-safe extension data plane and land only after MODULES-001 is reactivated.

## Exit Criteria

### Provider-safe core (no MODULES-001 dependency)

- [ ] Magic-byte reads and cache generation use provider-backed access.
- [ ] NodeEntry / Location-native rows document synchronous fields and lazy metadata guarantees.
- [ ] Nodes, metadata, and previews provide accessible labels without extra filesystem calls.
- [ ] Text, code, video, audio, markdown, document, hex, and binary payloads remain renderer-neutral.
- [ ] Thumbnail disk cache keys use stable identity or content hashes.

### Extension host (after MODULES-001)

- [ ] Manifest-declared preview and metadata providers register through the core host once MODULES-001 exists.
- [ ] Extension preview and metadata status uses structured core events once MODULES-001 exists.
