---
id: API-007
title: Retire the FileNode compatibility row and NodeId-keyed internals
status: To Do
priority: High
type: Refactor
parent: API-004
milestone: "0.3.0"
depends_on: [API-006]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Makes NodeEntry with LocationRef identity the only row contract and removes NodeId keying from internals."
tags: [api, nodeid, location]
last_updated: 2026-07-08
---

## Summary

Remove the FileNode compatibility row and the internal plumbing that keys on NodeId: the modules/compat translation layer, registry and cache identity, and pipeline row types. NodeEntry drops its NodeId field and carries LocationRef as its only identity. Stage per subsystem if the diff approaches the change-size limit.

## Acceptance Criteria

- [ ] NodeEntry carries LocationRef identity only; the FileNode row and its pipeline usages are gone.
- [ ] The compat translation module and NodeId-keyed registry and cache paths are removed.
- [ ] The full filer-core suite passes with row assertions migrated, not deleted.
