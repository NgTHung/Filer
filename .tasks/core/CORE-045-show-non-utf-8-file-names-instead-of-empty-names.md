---
id: "CORE-045"
title: "Show non-UTF-8 file names instead of empty names"
status: "To Do"
priority: "Medium"
type: "Bug"
milestone: "0.3.1"
parent: "core:CORE-027"
risk: "Low"
impact: "Files whose names are not valid UTF-8 list with an empty name, sort first, and escape hidden-file filtering."
tags: ["bug", "local", "model", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

NodeEntry::from_metadata and NodeEntry::from_dir_entry in crates/filer-core/src/model/node.rs convert the file name with to_str and fall back to an empty string, so a name that is not valid UTF-8 lists as "". Linux allows arbitrary bytes in names and Windows allows unpaired surrogates, and both local listing paths in crates/filer-core/src/vfs/local_listing.rs use these constructors. The empty name displays blank, sorts first, never matches name filters or search, and on Unix a non-UTF-8 dotfile is not marked hidden because hidden detection reads the empty name. The Location keeps the original path, so the file stays addressable. Derive the display name with a lossy conversion through one shared helper, and detect Unix hidden files from the raw name bytes. CORE-014 fixed the related NodeId panic on non-UTF-8 paths. ADR 0002's location tie-breaker keeps the name order total when two names convert to the same display name.

## Acceptance Criteria

- [ ] One helper derives an entry's display name from its OS file name, replacing invalid sequences with U+FFFD, and from_metadata and from_dir_entry both use it.
- [ ] Unix hidden detection reads the raw name bytes, so a non-UTF-8 name that starts with a dot is hidden.
- [ ] A Linux test lists a directory with non-UTF-8 names through the public scan command and asserts each entry has a non-empty display name and a location that resolves to the original path.
- [ ] A Linux test sorts and pages two non-UTF-8 names that convert to the same display name, one row per page, and receives each file exactly once.
- [ ] A cfg(windows) test covers a name with an unpaired surrogate and passes on a Windows machine.
