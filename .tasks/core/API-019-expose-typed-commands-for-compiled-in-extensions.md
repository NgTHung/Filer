---
id: "API-019"
title: "Expose typed commands for compiled-in extensions"
status: "To Do"
priority: "Medium"
type: "Refactor"
parent: "core:CORE-027"
milestone: "0.3.1"
depends_on: ["core:API-018"]
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["api", "extensions", "enhancement", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Replace the public string-plus-Any pairing with typed registration and command handles under ADR-0001. Migrate the existing Git decoration consumer as the first proof. Keep heterogeneous storage private and leave the future semantic envelope and host in MODULES-003/MODULES-005. Land the typed contract and misuse checks before migrating Git and examples.

## Acceptance Criteria

- [ ] A compiled-in extension registers its command type during setup and receives a typed submission interface; a wrong payload type cannot compile through that interface.
- [ ] Callers cannot bypass the typed interface with an arbitrary public string/payload pair; unavailable registrations and invalid payload values produce structured failures.
- [ ] Session and request or operation correlation remains available to shared validation and error reporting, including Git failures before handler execution.
- [ ] Git decoration public integration tests use the typed path; compile-fail rustdoc coverage proves type mismatch rejection and runtime tests cover unavailable handlers and invalid values.
- [ ] No WASM host, dynamic binary loading, or new wire protocol is introduced; cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
