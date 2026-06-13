---
id: CORE-024
title: Restore diagnostic error handling in dispatchers and providers
status: To Do
priority: Medium
type: Refactor
parent: CORE-004
risk: Low
impact: "Silently dropped commands and panic-on-call providers erode the diagnostic trail and the no-panic rule."
tags: [core, audit, remediation, reliability]
last_updated: 2026-06-13
---

## Summary

The per-module command dispatchers still drop sends with let _ = tx.send(...) instead of the crate's own send_or_warn helper, so a command to a closed actor channel vanishes with no log and no error to the caller; this affects operations/mod.rs, watch/mod.rs, and search/mod.rs. Route these through send_or_warn to restore a diagnostic trail with no happy-path change. Fold in three smaller error-handling fixes from the audit: replace the code.rs:61 theme fallback unwrap with a graceful unstyled fallback; replace the todo!() bodies in the feature-gated remote providers (s3, webdav, ftp, kubernetes, fuse) with Err(unsupported_operation(...)) so an enabled-but-incomplete provider degrades instead of panicking; and correct the copy-paste panic message at operations/mod.rs:61 that names ScanModule inside the operations module.

## Acceptance Criteria

- [ ] The command-dispatcher sends in operations/mod.rs, watch/mod.rs, and search/mod.rs route through send_or_warn.
- [ ] The code.rs theme fallback returns an unstyled string instead of unwrapping, and the operations/mod.rs:61 panic message names the correct module.
- [ ] The feature-gated remote providers return unsupported_operation errors instead of todo!() panics, verified with the relevant feature flag enabled.
