---
id: "OPS-006"
title: "Complete cross-device directory moves through shared transfer logic"
status: "To Do"
priority: "Medium"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:REL-015"]
rules: ["PROVIDER-ACCESS", "SESSION-BOUNDARY"]
risk: "Medium"
tags: ["operations", "providers", "bug", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

The cross-device move fallback calls a file-only provider copy even for directories. Share recursive transfer behavior with copy so a directory move can complete and failure preserves the source. Keep this separate from queue admission and recovery policy.

## Acceptance Criteria

- [ ] A real cross-device test where available, plus a contract-faithful provider test returning a cross-device rename error, moves a directory tree with nested files correctly.
- [ ] The source is removed only after the complete copy succeeds; failed or cancelled copies retain it and report known destination changes without claiming rollback.
- [ ] Copy and move share transfer traversal and preflight, preserve operation correlation and cache invalidation, and do not follow a failed copy with source deletion.
- [ ] cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
