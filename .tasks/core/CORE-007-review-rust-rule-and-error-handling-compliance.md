---
id: CORE-007
title: Review Rust-rule and error-handling compliance
status: Done
priority: High
type: Refactor
parent: CORE-004
risk: Medium
impact: "Banned unwrap/expect and silent error swallowing undermine the reliability priority."
tags: [core, audit, reliability]
last_updated: 2026-06-13
---

## Summary

Audit the 19 production unwrap/expect sites, needless clones, Result + ? usage, silent error swallowing, and error-context completeness against AGENTS.md Rust rules.

## Acceptance Criteria

- [x] Report at docs/reviews/filer-core/rust-rules.md classifies each unwrap/expect site as justified-and-tested or a violation.
- [x] Silent error-swallowing and needless-clone hotspots are listed with file:line.
- [x] Follow-up task candidates are listed for the violations worth fixing.
