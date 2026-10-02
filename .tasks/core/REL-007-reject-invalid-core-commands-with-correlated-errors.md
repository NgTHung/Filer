---
id: "REL-007"
title: "Reject invalid core commands with correlated errors"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:REL-015"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["api", "validation", "errors", "bug", "ready-for-agent"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-10-02"
---

## Summary

Make command rejection observable under ADR-0001. The router currently rejects unknown Sessions but logs and drops commands with no registered handler; value checks are spread across handlers. Inventory public command classes, extract shared validation where it repeats, and test each rejection before changing dispatch. Native input validation is the scope; restricted-Session authorization remains deferred in REL-010.

## Acceptance Criteria

- [ ] Unknown Sessions, malformed command values, unresolved or unsupported targets, and unavailable command handlers produce structured failures carrying every supplied correlation identity.
- [ ] Rejections never disappear into logging alone and never perform mutation or the rejected filesystem action; required target resolution may still report provider errors.
- [ ] Transport/channel submission is distinguished from mutation acceptance so callers cannot interpret a successful send as queue admission.
- [ ] Validation rules shared by command classes have one owner; public-interface tests prove representative invalid commands and valid commands take the intended routes.
- [ ] Documentation names the supported native permission model without promising restricted policy enforcement; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.

- [ ] Create and Rename accept exactly one native filename component; empty names, dot/parent components, rooted names, embedded separators, and platform-invalid names reject before writes. Public tests prove ../escaped.txt cannot escape its parent and valid native names remain supported.
- [ ] Transfer preflight from REL-015 is shared by direct command validation and execution; rejection keeps its correlation and never truncates an existing object. OPS-004 owns rechecking targets when queued work starts.
- [ ] Land shared native-value validation before router rejection coverage in separate test-first commits; mutation acceptance events are implemented by OPS-004.
