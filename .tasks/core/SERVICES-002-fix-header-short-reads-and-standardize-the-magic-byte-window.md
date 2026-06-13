---
id: SERVICES-002
title: Fix header short reads and standardize the magic-byte window
status: To Do
priority: High
type: Bug
risk: Medium
impact: "Fixes missing magic-byte detection for files smaller than the header window and centralizes the window size."
tags: [mime, vfs, bugfix]
last_updated: 2026-06-13
---

## Summary

LocalFs read_header uses read_exact, which returns UnexpectedEof for files smaller than n_bytes, so small files silently get no magic-byte detection (the resize branch is dead code). Replace it with a read that returns the bytes actually available, introduce one named constant for the magic-byte window set to 4096, and use it at every call site. 4096 matches the OS page/block size so it costs the same single read as 512 on local FS while covering deeper magic offsets and leaving room for text-vs-binary heuristics.

## Acceptance Criteria

- [ ] LocalFs read_header returns the available bytes for files smaller than the requested size instead of erroring.
- [ ] A single named constant defines the magic-byte window and is used by the registry and previewer call sites.
- [ ] The magic-byte window is 4096 bytes.
- [ ] A regression test covers a file smaller than the window producing a correct detection result.
- [ ] Duplicated 512 literals are removed from the detection call sites.
