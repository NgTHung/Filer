---
id: VFS-001
title: Route segmented provider locations
status: Done
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS, SESSION-BOUNDARY]
risk: High
impact: "Extends navigation across archives and layered provider locations."
tags: [vfs, location, archive]
last_updated: 2026-06-24
---

## Summary

Build provider routing and navigation over ordered Location segments.

## Acceptance Criteria

- [x] Segmented descriptors carry capability, display, and target metadata.
- [x] Archive members are navigable provider locations rather than preview-only entries.
- [x] Nested archive and provider layers resolve in order.
- [x] Unsupported routes return structured provider errors.
