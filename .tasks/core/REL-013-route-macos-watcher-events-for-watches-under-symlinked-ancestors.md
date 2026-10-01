---
id: "REL-013"
title: "Route macOS watcher events for watches under symlinked ancestors"
status: "To Do"
priority: "High"
type: "Bug"
milestone: "0.3.1"
risk: "Medium"
impact: "Watching a folder under /tmp, /var, or any symlinked ancestor on macOS delivers no change events, so views go stale."
tags: ["bug", "needs-triage", "watcher", "portability", "reliability"]
last_updated: "2026-10-01"
---

## Summary

The first macOS CI run (GitHub Actions run 36819277073) failed eight watcher tests in modules/watcher_test.rs because no FsChanged event arrived. The watcher routes a provider change to a watch only when change.path.starts_with(entry.path) in modules/watch/watcher.rs. FSEvents reports canonical paths, so a watch registered as /var/folders/... receives changes for /private/var/folders/... and drops them all. Reproduce that on macOS first, then make routing compare paths in one form without changing the location reported in FsChanged events. The tests carry a macOS-only ignore that names this task until it lands.

## Acceptance Criteria

- [ ] A test reproduces dropped events for a watch registered through a symlinked ancestor, and fails on macOS before the fix.
- [ ] Watch events route to sessions whose watch path reaches the changed path through a symlinked ancestor, and FsChanged still reports the watched location as requested.
- [ ] The macOS ignore on the watcher tests is removed and they pass in CI on Linux, Windows, and macOS.
