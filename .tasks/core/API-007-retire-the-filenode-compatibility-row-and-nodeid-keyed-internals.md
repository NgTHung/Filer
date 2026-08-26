---
id: API-007
title: Retire the FileNode compatibility row and NodeId-keyed internals
status: Done
priority: High
type: Epic
parent: API-004
milestone: "0.3.0"
depends_on: [API-006]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Makes NodeEntry with LocationRef identity the only row contract and removes NodeId keying from internals."
tags: [api, nodeid, location]
last_updated: 2026-08-26
---

## Summary

Coordinate the staged removal of the FileNode compatibility row and NodeId-keyed internals. The child tasks migrate navigation identity, provider rows, pipeline and cache storage, then remove the remaining registry bridges without exceeding the repository's review-size guidance.

## Exit Criteria

- [x] API-014, API-015, API-016, and API-017 are Done.
- [x] NodeEntry is the only row contract and carries LocationRef as its only identity.
- [x] No production internal depends on NodeId; only the definition and API-008 deletion pin remain.
- [x] The full filer-core suite and ignored stress tests pass with assertions migrated, not deleted.
