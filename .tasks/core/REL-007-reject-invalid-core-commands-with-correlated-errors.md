---
id: "REL-007"
title: "Reject invalid core commands with correlated errors"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["api", "validation", "errors", "bug", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Make command rejection observable under ADR-0001. The router currently rejects unknown Sessions but logs and drops commands with no registered handler; value checks are spread across handlers. Inventory public command classes, extract shared validation where it repeats, and test each rejection before changing dispatch. Native input validation is the scope; restricted-Session authorization remains deferred in REL-010.

## Acceptance Criteria

- [ ] Unknown Sessions, malformed command values, unresolved or unsupported targets, and unavailable command handlers produce structured failures carrying every supplied correlation identity.
- [ ] Rejections never disappear into logging alone and never perform mutation or the rejected filesystem action; required target resolution may still report provider errors.
- [ ] Transport/channel submission is distinguished from mutation acceptance so callers cannot interpret a successful send as queue admission.
- [ ] Validation rules shared by command classes have one owner; public-interface tests prove representative invalid commands and valid commands take the intended routes.
- [ ] Documentation names the supported native permission model without promising restricted policy enforcement; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
