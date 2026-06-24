---
id: PROVIDER-009
title: Lock provider-backed archive and location contracts
status: Done
priority: Medium
type: TestDebt
parent: PROVIDER-002
depends_on: [PROVIDER-008]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: Medium
impact: "Prevents archive provider and Location contract regressions."
tags: [provider, vfs, testing]
last_updated: 2026-06-24
---

## Summary

Add contract tests proving provider profiles, Location identity, and archive segmented routing stay provider-backed.

## Acceptance Criteria

- [x] Location descriptors round-trip for local, profile, ephemeral, segmented archive, and virtual-segment cases.
- [x] Segmented archive listing uses ArchiveFs-backed behavior.
- [x] No concrete remote-provider stub is added.
