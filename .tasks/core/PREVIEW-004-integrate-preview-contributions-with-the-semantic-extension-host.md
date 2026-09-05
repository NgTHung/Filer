---
id: "PREVIEW-004"
title: "Integrate preview contributions with the semantic extension host"
status: "To Do"
priority: "Medium"
type: "Epic"
parent: "core:PREVIEW-001"
milestone: "0.4.0"
depends_on: ["core:PREVIEW-003", "core:MODULES-003", "core:MODULES-004", "core:MODULES-005"]
rules: ["SEMANTIC-EXTENSION-OUTPUT", "SESSION-BOUNDARY"]
risk: "Medium"
tags: ["core"]
last_updated: "2026-09-05"
---

## Summary

Own extension preview and metadata registration after provider-safe built-ins and the envelope, subscriptions, and trusted host exist. Refine manifest registration and event delivery into separate implementation tickets when those contracts land; do not introduce another preview host.

## Exit Criteria

- [ ] Manifest-declared preview and metadata providers register through the shared trusted host with session/provider constraints.
- [ ] Preview and metadata status uses structured semantic events, and a failed or slow contributor does not block directory delivery.
- [ ] Implementation tickets pin registration, cancellation, invalidation, and failure behavior before code changes.
