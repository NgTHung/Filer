---
id: API-010
title: Migrate module actor tests to Location-native identity
status: Done
priority: High
type: TestDebt
parent: API-005
milestone: "0.3.0"
depends_on: [API-009]
rules: [CORE-LIBRARY]
risk: Medium
impact: "Ports navigator, scanner, search, operator, and watcher tests off NodeId so actor coverage survives NodeId removal."
tags: [api, nodeid, location, testing]
last_updated: 2026-08-08
---

## Summary

Port the navigator, scanner, search, operator, and watcher module tests under `filer-core/src/tests/modules` to the Location-native commands, events, and assertions introduced by API-005/API-009. Preserve the existing behavior coverage and use the shared API-009 fixtures for provider-shaped setup. This task changes tests and test fixtures only; previewer, router, and top-level integration tests belong to API-011/API-012. Compatibility-only tests may remain when a production command or event has no native replacement yet, but each such test must be isolated and explicitly marked for API-006.

## Acceptance Criteria

- [x] Behavior assertions in the navigator, scanner, search, operator, and watcher module tests use LocationRef or NodeEntry identity and the corresponding Location-native command/event variants; FileNode remains only where an FsProvider mock requires that provider-shaped input.
- [x] Every intentional NodeId/FileNode compatibility pin is isolated and explicitly labeled for API-006; no unmarked compatibility identity assertion remains.
- [x] The full filer-core suite passes with no reduction in test count.
