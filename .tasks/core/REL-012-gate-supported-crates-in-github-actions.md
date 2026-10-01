---
id: "REL-012"
title: "Gate supported crates in GitHub Actions"
status: In Progress
priority: "High"
type: "TestDebt"
risk: "Low"
impact: "Runs the checks every change already needs on Linux, Windows, and macOS, so platform-specific behavior such as VFS-002's Windows paths gets executed evidence."
tags: ["reliability", "testing", "tooling", "portability", "enhancement", "ready-for-agent"]
last_updated: 2026-10-01
---

## Summary

The repository has no checked-in CI, so its tests have only run on maintainers' Linux machines and Windows-only code is compiled but never executed. Add a GitHub Actions workflow that runs on pushes to main and on pull requests. It covers rustfmt, Clippy with warnings denied, and tests for every root workspace crate except filer-app, on Linux, Windows, and macOS. It also runs the filer-core feature checker's minimal and all cases, the isolated benchmark protocol package, and taskroot validation. filer-app stays excluded until UI-012 provides isolated validation targets, and the workflow says so where it excludes it.

## Acceptance Criteria

- [ ] Pushes to main and pull requests run rustfmt, Clippy with warnings denied, and tests with --locked for the root workspace except filer-app on Linux, Windows, and macOS.
- [ ] The workflow runs the filer-core feature checker's minimal and all cases, the benchmark protocol package checks, and taskroot validate.
- [ ] The filer-app exclusion is explained in the workflow and points at UI-012.
- [ ] A first run on GitHub passes, or each platform failure it exposes is fixed or tracked in its own task.

## Rationale

The 2026-09-30 project audit found no checked-in CI and asked for an automated gate over supported workspace crates and selected feature cases.

