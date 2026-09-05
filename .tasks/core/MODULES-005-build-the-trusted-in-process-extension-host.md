---
id: MODULES-005
title: Build the trusted in-process extension host
status: Deferred
priority: High
type: Feature
parent: MODULES-001
milestone: "0.4.0"
depends_on: [MODULES-003, API-019]
rules: [WIRE-SAFE-EXTENSIONS, SESSION-BOUNDARY]
risk: High
impact: "Owns trusted compiled-in contribution registration and manifest validation through Core contracts."
tags: [extensions, host]
last_updated: 2026-09-05
---

## Summary

Build the trusted compiled-in host using API-019 typed registration and the
startup freeze from ADR-0001. Validate manifests, register contributions before
client command admission, and keep tracing and recoverable contribution failures
inside Core contracts. Reuse MODULES-003 envelopes for portable semantic output;
do not expose arbitrary string/payload pairing to typed callers.

Restricted-Session authorization is deferred to REL-010. Manifest declarations
must not be presented as enforced isolation until that contract exists. Core
policy cannot grant filesystem rights unavailable to the OS user. No WASM host,
marketplace, independently installed binary, or live module replacement belongs
to this stage.

## Acceptance Criteria

- [ ] Compiled-in contributions register through API-019 before startup completes and cannot replace running built-in handlers.
- [ ] Validated manifests and MODULES-003 semantic output retain Session and correlation identity; contribution errors are recoverable without silently losing requests.
- [ ] Public integration tests prove Git registration and failure behavior without claiming restricted authorization or process isolation.

## Rationale

Staged decomposition of MODULES-001; reactivate when MILESTONE-005 (0.4.0) work begins.
