---
id: PROVIDER-004
title: Redact provider credentials from Debug and tracing output
status: Done
priority: Medium
type: Feature
milestone: "0.3.0"
rules: [PROVIDER-ACCESS]
risk: Low
impact: "Prevents provider credentials from leaking into debug logs and command-path tracing."
tags: [provider, secrets, reliability]
last_updated: 2026-06-29
---

## Summary

The audit that created this task named remote provider config structs that no longer exist in current filer-core. Keep the current ProviderProfile boundary secret-free, and add a small runtime secret wrapper for future provider configs so credentials cannot leak through Debug or tracing output.

## Acceptance Criteria

- [x] ProviderProfile Debug and serialization stay free of credential, password, secret, and token values.
- [x] Runtime provider secret fields can use a shared wrapper whose Debug output renders a fixed redaction marker.
- [x] Secret fields named password, secret_key, session_token, bearer_token, and private_key are covered by a regression test that proves their Debug output contains no secret value.
