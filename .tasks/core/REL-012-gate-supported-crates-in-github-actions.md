---
id: "REL-012"
title: "Gate supported crates in GitHub Actions"
status: Done
priority: "High"
type: "TestDebt"
risk: "Low"
impact: "Runs the checks every change already needs on Linux, Windows, and macOS, so platform-specific behavior such as VFS-002's Windows paths gets executed evidence."
tags: ["reliability", "testing", "tooling", "portability", "enhancement", "ready-for-agent"]
last_updated: 2026-10-01
---

## Summary

The repository has no checked-in CI, so its tests have only run on maintainers' Linux machines and Windows-only code is compiled but never executed. Add a GitHub Actions workflow that maintainers start manually. It runs Clippy with warnings denied and all-features tests for every root workspace crate except filer-app on Linux, Windows, and macOS. On Linux it also checks rustfmt and runs the filer-core feature checker's minimal and default cases, the isolated benchmark protocol package, and taskroot validation. filer-app stays excluded until UI-012 provides isolated validation targets, and the workflow says so where it excludes it.

## Acceptance Criteria

- [x] A manual run runs Clippy with warnings denied and tests with --locked for the root workspace except filer-app on Linux, Windows, and macOS.
- [x] The workflow runs the filer-core feature checker's minimal and default cases, rustfmt, the benchmark protocol package checks, and taskroot validate.
- [x] The filer-app exclusion is explained in the workflow and points at UI-012.
- [x] A first run on GitHub passes, or each platform failure it exposes is fixed or tracked in its own task.

## Rationale

The 2026-09-30 project audit found no checked-in CI and asked for an automated gate over supported workspace crates and selected feature cases.

