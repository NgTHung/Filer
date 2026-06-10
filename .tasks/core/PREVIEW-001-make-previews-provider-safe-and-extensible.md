---
id: PREVIEW-001
title: Make previews provider-safe and extensible
status: To Do
priority: Medium
type: Epic
depends_on: [MODULES-001]
rules: [PROVIDER-ACCESS, ACTOR-LONG-WORK, SEMANTIC-EXTENSION-OUTPUT]
risk: Medium
impact: "Changes preview, metadata, and cache contracts used by clients."
tags: [preview, metadata, provider]
last_updated: 2026-06-06
---

## Summary

Remove local-path assumptions and define stable lazy metadata and preview payloads.

## Exit Criteria

- [ ] Magic-byte reads and cache generation use provider-backed access.
- [ ] FileNode documents synchronous fields and lazy metadata guarantees.
- [ ] Nodes, metadata, and previews provide accessible labels without extra filesystem calls.
- [ ] Text, code, video, audio, markdown, document, hex, and binary payloads remain renderer-neutral.
- [ ] Thumbnail disk cache keys use stable identity or content hashes.
- [ ] Manifest-declared preview and metadata providers register through the core host.
- [ ] Extension preview and metadata status uses structured core events.
