---
id: "VFS-002"
title: "Enumerate local directory pages in one blocking task"
status: "To Do"
priority: "High"
type: "Refactor"
milestone: "0.3.1"
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK"]
risk: "Medium"
impact: "Removes per-entry blocking-pool hops from metadata listings and prepares a native Windows enumeration path."
tags: ["core", "performance", "vfs", "enhancement", "ready-for-agent"]
last_updated: "2026-09-23"
---

## Summary

LocalFs lists through tokio::fs::read_dir. Metadata listings call DirEntry::metadata per entry in vfs/local_listing.rs, and each call is a separate blocking-pool hop. On Windows the enumeration record already carries size, timestamps, and attributes, so that hop is pure overhead. Enumerate each page inside one blocking task and take metadata from the enumeration record where the platform provides it. Measure std::fs::read_dir in one blocking task first because it is portable. Add a windows-sys path, such as FindFirstFileExW with FindExInfoBasic and FIND_FIRST_EX_LARGE_FETCH or GetFileInformationByHandleEx directory classes, only if it beats the std path on the Windows benchmark. Preserve DirectoryCursor continuation, ProviderCx cancellation and timeouts, and current symlink and junction classification.

## Acceptance Criteria

- [ ] Fast and Metadata page listings enumerate and read metadata for one page inside one blocking task with no per-entry blocking hop.
- [ ] Tests prove row kind, size, timestamps, hidden state, and symlink or junction classification match current behavior on Linux and Windows, including cancellation and cursor continuation.
- [ ] Same-machine before/after runs on Windows NTFS and Linux record Metadata first-page time and allocations.
- [ ] A native Windows enumeration path and its dependency land only when the Windows benchmark shows a gain over std enumeration in one blocking task; otherwise the measured result is recorded and no dependency is added.

## Rationale

Added on 2026-09-23 after comparing Filer with Filesmash, a native Win32 file manager. Rust reaches the same Win32 enumeration calls, so listing parity depends on batching and API choice, not language.

