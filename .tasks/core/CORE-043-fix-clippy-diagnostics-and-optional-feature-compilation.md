---
id: "CORE-043"
title: "Fix Clippy diagnostics and optional-feature compilation"
status: Done
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
risk: "Medium"
impact: "Restores clean filer-core builds and lint checks across optional feature configurations."
tags: ["core", "bug", "quality", "testing", "remediation", "ready-for-agent"]
last_updated: 2026-09-06
---

## Summary

Resolve filer-core Clippy diagnostics and compiler errors exposed when optional features are disabled or enabled independently. This follows the CORE-024 review and belongs to the active 0.3.1 core maintenance scope.

Reproduced on 2026-09-05: cargo test -p filer-core --no-default-features --lib preview fails with unresolved zip imports in src/vfs/archive.rs, src/tests/modules/scanner_test/scanner_cache_tests.rs, and src/tests/vfs/vfs_test/segmented_location_tests.rs. The same build reports unused imports and parameters in feature-disabled metadata and preview providers. cargo clippy -p filer-core --all-targets --features preview-code -- -D warnings fails in production and test code, including redundant closures, argument counts, collapsible conditionals, a large enum variant, Option map/flatten, and test fixture lint violations. These examples are starting evidence, not the complete diagnostic inventory.

Use filer-core/Cargo.toml as the feature inventory. Validate no default features, default features, each declared non-default feature individually with defaults disabled, defaults plus preview-code, defaults plus preview, and all features. Include native prerequisites for metadata-archive-rar in the recorded validation setup. Keep optional dependencies optional and align dependency use, module exports, implementations, and test fixtures with their feature gates.

Preserve public behavior and enabled-feature coverage. Fix lint causes without blanket allow attributes, deleting tests, or enabling every dependency to make reduced builds pass. Any narrow lint exception must explain why the existing design is appropriate and retain meaningful coverage. Coordinate structural fixes with CORE-035/036 and fixture changes with CORE-037, which already own broader refactors. Stage implementation within repository change-size limits after inspecting the actual diagnostic inventory.

## Acceptance Criteria

- [x] Record a repeatable feature matrix derived from filer-core/Cargo.toml covering no defaults, defaults, each declared non-default feature independently without defaults, defaults plus preview-code, defaults plus preview, and all features.
- [x] For every matrix entry, cargo check -p filer-core --all-targets and cargo test -p filer-core --all-targets --no-run succeed with the corresponding feature flags; optional dependencies remain optional.
- [x] For every matrix entry, cargo clippy -p filer-core --all-targets with the corresponding feature flags and -- -D warnings succeeds, covering compiler warnings and Clippy diagnostics in library, tests, examples, and benchmarks.
- [x] Add regression checks before fixes for broken feature configurations, and run applicable tests across the matrix, including archive behavior when enabled and CORE-024 preview fallback tests when preview-code is enabled.
- [x] Add a repository check script or existing automation integration that runs the matrix, propagates failures, and documents commands, toolchain, and native prerequisites so feature isolation regressions are detected.
- [x] Record passing validation evidence for every matrix entry and preserve public contracts and existing test coverage; document any narrowly scoped lint exception with its rationale.


## Implementation Notes

Archive listing now shares one feature-gated path. Without metadata-archive,
both owned and borrowed archive providers return a structured unsupported error
before file I/O. Metadata imports, helpers, and format-specific tests follow
their feature gates. Cargo dependency declarations remain unchanged.

Clippy fixes simplify node construction, group borrowed scan resources and event
correlation, reuse operation progress scope, and share test log types. Scanner
inputs live in a new module. CORE-035/036 retain their broader actor decomposition
scope; CORE-037 retains fixture consolidation beyond the shared log type.

Two narrow lint expectations preserve public APIs: SearchCommand keeps its
payload inline to avoid a per-request allocation, and Pipeline::add retains its
stage-builder meaning. Each expectation carries its rationale in source.

The failing regression and matrix runner landed in cbe6616 before the feature
fix in 24672f7. Mechanical lint fixes and structural changes landed separately
in 234fa3e and 6d309dc. Standards and spec reviews found no actionable findings.

## Validation Evidence

Validated on 2026-09-06 with rustc 1.97.1, cargo 1.97.1, Clippy 0.1.97,
Python 3.13.5, and Debian Linux x86_64. The native C/C++ compiler, linker,
make, and pkg-config were available, including for metadata-archive-rar.

Use python3 filer-core/tests/check_features.py to repeat the matrix. Its 16
configurations passed all five phases: all-target check, all-target test
compilation, all-target Clippy with -D warnings, library/integration tests,
and doctests. All 80 command logs were checked for errors and warnings; none
remain. Each configuration passed 14 doctests. Ten existing ignored stress
tests remain opt-in; benchmarks compiled without being executed.

| Configuration | Runtime tests passed |
| --- | ---: |
| minimal | 814 |
| default | 848 |
| metadata-image | 820 |
| metadata-audio | 819 |
| metadata-video | 818 |
| metadata-document | 817 |
| metadata-archive | 830 |
| metadata-archive-rar | 830 |
| metadata | 848 |
| preview-code | 816 |
| preview-image | 814 |
| preview | 816 |
| all-features | 850 |
| default-preview-code | 850 |
| default-preview | 850 |
| all | 850 |

The minimal build pins unsupported archive listing before I/O. Configurations
with preview-code pass both CORE-024 fallback regressions. Archive-enabled
configurations retain archive listing and metadata tests. The matrix runner
returned failure for the original reduced-feature build errors before the fix.
Task validation, diff whitespace checks, and formatting checks for the new Rust
files passed. Logs are available locally under target/feature-matrix; commands
and prerequisites are documented in filer-core/tests/README.md.
