---
id: "VFS-004"
title: "List existing empty ZIP directories successfully"
status: "To Do"
priority: "Medium"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["PROVIDER-ACCESS"]
risk: "Medium"
tags: ["archive", "bug", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

An explicit empty ZIP directory is shown by its parent but opening it returns PathNotFound because existence is inferred from child count. Track existence separately so an empty directory and a missing member have distinct results.

## Acceptance Criteria

- [ ] ArchiveFs returns an empty successful listing for an explicit empty directory and an empty archive root.
- [ ] A missing member still returns PathNotFound; implicit directories and populated directories keep their existing behavior.
- [ ] Archive provider tests cover each case with real ZIP fixtures, and cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
