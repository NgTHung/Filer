---
id: CORE-009
title: Review VFS and provider abstraction
status: Done
priority: High
type: Design
parent: CORE-004
rules: [PROVIDER-ACCESS]
risk: High
impact: "The provider surface determines whether remote, archive, and timeout-bound access can land without core churn."
tags: [core, audit, vfs, provider]
last_updated: 2026-06-13
---

## Summary

Review the FsProvider trait surface, native vs fallback paging, the capability model, the timeout-context gap (PROVIDER-001), and segmented/archive routing readiness.

## Acceptance Criteria

- [x] Report at docs/reviews/filer-core/vfs-provider.md evaluates the trait surface and paging strategies.
- [x] The timeout-context gap and segmented/archive routing readiness are assessed against the planned providers.
- [x] Follow-up task candidates are listed for abstraction gaps that would force later rework.
