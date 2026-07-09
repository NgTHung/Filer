---
id: MODULES-005
title: Build the trusted in-process extension host
status: Deferred
priority: High
type: Feature
parent: MODULES-001
milestone: "0.4.0"
depends_on: [MODULES-003]
rules: [WIRE-SAFE-EXTENSIONS, SESSION-BOUNDARY]
risk: High
impact: "Owns manifest validation and permission mapping so extension output stays core-authoritative."
tags: [extensions, host]
last_updated: 2026-07-09
---

## Summary

Build the trusted in-process host that loads validated extension manifests and maps declared permissions to sessions and provider capabilities. Scoped filesystem calls, contribution registration, tracing, and recoverable failure handling stay core-authoritative. No WASM sandbox and no marketplace; trust comes from in-process compilation, not isolation.

## Rationale

Staged decomposition of MODULES-001; reactivate when MILESTONE-005 (0.4.0) work begins.

