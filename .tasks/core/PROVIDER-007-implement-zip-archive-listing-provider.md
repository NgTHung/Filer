---
id: PROVIDER-007
title: Implement ZIP archive listing provider
status: Done
priority: Medium
type: Feature
parent: PROVIDER-002
depends_on: [PROVIDER-006]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: Medium
impact: "Turns archive navigation into provider-backed VFS behavior."
tags: [provider, vfs, archive]
last_updated: 2026-06-24
---

## Summary

Replace the ArchiveFs stub with a read-only ZIP-backed provider that lists archive directories.

## Acceptance Criteria

- [x] ArchiveFs lists ZIP root and child directories through FsProvider::list.
- [x] ArchiveFs reports read true, write false, watch false, and search false.
- [x] ArchiveFs returns structured errors for member reads and write operations in this stage.
