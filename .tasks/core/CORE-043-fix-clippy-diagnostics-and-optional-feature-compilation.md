---
id: "CORE-043"
title: "Fix Clippy diagnostics and optional-feature compilation"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
risk: "Medium"
impact: "Restores clean filer-core builds and lint checks across optional feature configurations."
tags: ["core", "bug", "quality", "testing", "remediation", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Resolve filer-core Clippy diagnostics and compiler errors exposed when optional features are disabled or enabled independently. This follows the CORE-024 review and belongs to the active 0.3.1 core maintenance scope.

Reproduced on 2026-09-05: cargo test -p filer-core --no-default-features --lib preview fails with unresolved zip imports in src/vfs/archive.rs, src/tests/modules/scanner_test/scanner_cache_tests.rs, and src/tests/vfs/vfs_test/segmented_location_tests.rs. The same build reports unused imports and parameters in feature-disabled metadata and preview providers. cargo clippy -p filer-core --all-targets --features preview-code -- -D warnings fails in production and test code, including redundant closures, argument counts, collapsible conditionals, a large enum variant, Option map/flatten, and test fixture lint violations. These examples are starting evidence, not the complete diagnostic inventory.

Use filer-core/Cargo.toml as the feature inventory. Validate no default features, default features, each declared non-default feature individually with defaults disabled, defaults plus preview-code, defaults plus preview, and all features. Include native prerequisites for metadata-archive-rar in the recorded validation setup. Keep optional dependencies optional and align dependency use, module exports, implementations, and test fixtures with their feature gates.

Preserve public behavior and enabled-feature coverage. Fix lint causes without blanket allow attributes, deleting tests, or enabling every dependency to make reduced builds pass. Any narrow lint exception must explain why the existing design is appropriate and retain meaningful coverage. Coordinate structural fixes with CORE-035/036 and fixture changes with CORE-037, which already own broader refactors. Stage implementation within repository change-size limits after inspecting the actual diagnostic inventory.

## Acceptance Criteria

- [ ] Record a repeatable feature matrix derived from filer-core/Cargo.toml covering no defaults, defaults, each declared non-default feature independently without defaults, defaults plus preview-code, defaults plus preview, and all features.
- [ ] For every matrix entry, cargo check -p filer-core --all-targets and cargo test -p filer-core --all-targets --no-run succeed with the corresponding feature flags; optional dependencies remain optional.
- [ ] For every matrix entry, cargo clippy -p filer-core --all-targets with the corresponding feature flags and -- -D warnings succeeds, covering compiler warnings and Clippy diagnostics in library, tests, examples, and benchmarks.
- [ ] Add regression checks before fixes for broken feature configurations, and run applicable tests across the matrix, including archive behavior when enabled and CORE-024 preview fallback tests when preview-code is enabled.
- [ ] Add a repository check script or existing automation integration that runs the matrix, propagates failures, and documents commands, toolchain, and native prerequisites so feature isolation regressions are detected.
- [ ] Record passing validation evidence for every matrix entry and preserve public contracts and existing test coverage; document any narrowly scoped lint exception with its rationale.
