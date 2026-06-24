---
id: CORE-026
title: Split oversized filer-core test files
status: To Do
priority: Medium
type: TestDebt
parent: CORE-001
milestone: "0.3.0"
rules: [CORE-LIBRARY]
risk: Medium
impact: "Keeps core tests reviewable and prevents large inline test modules from growing production files."
tags: [core, tests, maintainability]
last_updated: 2026-06-24
---

## Summary

Split oversized filer-core test files by behavior seam and move inline tests out of production modules into the existing src/tests tree. Do not include services/mime/table.rs; SERVICES-003 owns the MIME detector and table migration.

## Acceptance Criteria

- [ ] command_router_test.rs, scanner_test.rs, operator_test.rs, search_test.rs, pipeline_test.rs, navigator_test.rs, vfs_test.rs, and metadata_test.rs are split so each resulting test module follows the project size guidance.
- [ ] Inline #[cfg(test)] test modules in production code are moved into filer-core/src/tests unless a short inline unit test has a documented locality reason.
- [ ] Shared fixtures, builders, mock providers, and async event wait helpers are reused instead of copied across the new test files.
- [ ] The split preserves test coverage and behavior; cargo test -p filer-core passes.
