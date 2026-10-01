---
id: "VFS-003"
title: "Return requested local locations from LocalFs metadata"
status: "To Do"
priority: "High"
type: "Bug"
milestone: "0.3.1"
risk: "Medium"
impact: "One local file can carry two identities, so selection, cache, and decoration lookups keyed by location can miss on macOS and through symlinked folders."
tags: ["bug", "needs-triage", "vfs", "location", "portability"]
last_updated: "2026-10-01"
---

## Summary

LocalFs::metadata builds its row through NodeEntry::from_path, which canonicalizes the path, while listings keep the path they enumerated. On macOS the temporary directory sits under /var, a symlink to /private/var, so the first macOS CI run (GitHub Actions run 36819277073) failed test_local_fs_metadata: metadata returned a /private/var location for a /var request. On Windows the second run (36820018191) showed the split for every path: canonicalization adds the \\?\ verbatim prefix and expands 8.3 short names such as RUNNER~1. The same split happens on any platform when a path goes through a symlinked folder. Decide whether metadata should report the requested location, matching listings, and keep any canonical form internal. The test carries a macOS and Windows ignore that names this task until it lands.

## Acceptance Criteria

- [ ] LocalFs::metadata and LocalFs listings report the same location for one entry reached through a symlinked ancestor, with a test that runs on every platform.
- [ ] Home-relative expansion in NodeEntry::from_path keeps working, with a test.
- [ ] The macOS and Windows ignore on test_local_fs_metadata is removed and it passes in CI on Linux, Windows, and macOS.
