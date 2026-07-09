---
id: CORE-024
title: Remove code preview theme unwrap fallback
status: To Do
priority: Medium
type: Refactor
parent: CORE-027
milestone: "0.3.1"
risk: Low
impact: "Theme fallback can panic when the theme set is empty, violating the no-panic rule on a preview path."
tags: [core, audit, remediation, reliability]
last_updated: 2026-07-09
---

## Summary

Most of the audit findings behind the original dispatcher/provider diagnostic task already landed: the module dispatchers in operations/mod.rs, watch/mod.rs, and search/mod.rs route every send through send_or_warn, and incomplete feature-gated VFS provider stubs are gone (re-verified 2026-07-08). Remaining work is the theme fallback unwrap at services/preview/providers/code.rs:61, which chains unwrap_or_else into unwrap and panics when the theme set is empty; replace it with a graceful unstyled fallback.

Milestone 0.3.1 (MILESTONE-004 draft). Tracked under CORE-027 post-audit remediation.

## Acceptance Criteria

- [x] The command-dispatcher sends in operations/mod.rs, watch/mod.rs, and search/mod.rs route through send_or_warn.
- [ ] The code.rs theme fallback returns an unstyled string instead of unwrapping, pinned by a test with an empty theme set.
- [x] Incomplete feature-gated VFS provider stubs no longer exist in filer-core.
