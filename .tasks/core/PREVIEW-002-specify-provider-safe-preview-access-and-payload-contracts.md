---
id: "PREVIEW-002"
title: "Specify provider-safe preview access and payload contracts"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:PREVIEW-001"
milestone: "0.4.0"
depends_on: ["milestones:MILESTONE-004"]
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK"]
risk: "Medium"
tags: ["core", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Inspect the current path-based PreviewProvider::generate, preview cache, metadata extractor, FsProvider reads, and Previewer cancellation flow. Specify the smallest provider-backed text/code slice and the access contract needed by remaining built-in providers. Preserve NodeEntry/Location identity and renderer-neutral payloads. This design does not require the extension host.

## Acceptance Criteria

- [ ] The design names synchronous row fields, lazy metadata guarantees, accessible labels, provider reads, timeout/cancellation behavior, and errors for unsupported reads.
- [ ] A first text/code slice has observable tests using a non-local provider double, with follow-on cache and other payload stages mapped to PREVIEW-003.
- [ ] PREVIEW-003 is decomposed into bounded implementation tickets with test-first criteria and concrete dependencies before any production change.
