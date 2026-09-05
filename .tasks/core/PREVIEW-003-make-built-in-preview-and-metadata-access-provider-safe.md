---
id: "PREVIEW-003"
title: "Make built-in preview and metadata access provider-safe"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "core:PREVIEW-001"
milestone: "0.4.0"
depends_on: ["core:PREVIEW-002"]
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["core"]
last_updated: "2026-09-05"
---

## Summary

Own the provider-safe core track of PREVIEW-001. PREVIEW-002 defines the access contract and splits this map into implementation stages before execution. Text/code access lands first; remaining payload and cache work follows provider capability evidence. Extension registration belongs to PREVIEW-004.

## Exit Criteria

- [ ] Magic-byte reads, metadata, and preview/cache generation use provider-backed Location identity and tested cancellation/error contracts.
- [ ] NodeEntry fields and lazy metadata guarantees are documented, and accessible labels require no extra filesystem calls.
- [ ] Text, code, video, audio, markdown, document, hex, and binary payloads stay renderer-neutral; thumbnail cache keys use stable identity or content hashes.
- [ ] All implementation children are Done with focused and full core checks.
