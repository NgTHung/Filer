---
id: CORE-024
title: Restore diagnostic error handling in dispatchers and providers
status: To Do
priority: Medium
type: Refactor
parent: CORE-001
risk: Low
impact: "Silently dropped commands and panic-on-call providers erode the diagnostic trail and the no-panic rule."
tags: [core, audit, remediation, reliability]
last_updated: 2026-07-08
---

## Summary

Most of the audit findings behind this task landed with later work: the module dispatchers in operations/mod.rs, watch/mod.rs, and search/mod.rs now route every send through send_or_warn, and the copy-paste panic message in operations/mod.rs is gone (re-verified 2026-07-08). What remains is the theme fallback unwrap at services/preview/providers/code.rs:61, which chains unwrap_or_else into unwrap and panics when the theme set is empty; replace it with a graceful unstyled fallback.

## Acceptance Criteria

- [x] The command-dispatcher sends in operations/mod.rs, watch/mod.rs, and search/mod.rs route through send_or_warn.
- [ ] The code.rs theme fallback returns an unstyled string instead of unwrapping, pinned by a test with an empty theme set.
- [x] Incomplete feature-gated VFS provider stubs no longer exist in filer-core.
