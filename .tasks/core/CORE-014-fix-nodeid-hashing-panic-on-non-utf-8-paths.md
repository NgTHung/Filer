---
id: CORE-014
title: Fix NodeId hashing panic on non-UTF-8 paths
status: Done
priority: High
type: Bug
parent: CORE-001
milestone: "0.3.0"
risk: High
impact: "Removes a panic on valid filesystem input that sits on the main scan path."
tags: [core, audit, remediation, reliability]
last_updated: 2026-07-04
---

## Summary

NodeId::from_path hashes path.to_str().unwrap(), which panics on any non-UTF-8 path. Such paths are legal on both Windows (unpaired surrogates) and Linux (arbitrary bytes), so a single such file in a scanned directory panics node-id hashing on the main scan path. Hash the raw OS bytes instead via path.as_os_str().as_encoded_bytes() (or the OsStr Hash impl), which never fails.

## Acceptance Criteria

- [x] NodeId::from_path hashes OS path bytes instead of the UTF-8 view and no longer calls unwrap.
- [x] A test builds a NodeId from a non-UTF-8 path (or an OsStr from invalid bytes) and asserts it does not panic and produces a stable id.
- [x] from_metadata and from_dir_entry, which share the same id path, are covered by the non-UTF-8 case.
