---
id: PROVIDER-004
title: Redact provider credentials from Debug and tracing output
status: To Do
priority: Medium
type: Feature
milestone: "0.3.0"
rules: [PROVIDER-ACCESS]
risk: Low
impact: "Prevents provider credentials from leaking into debug logs and command-path tracing."
tags: [provider, secrets, reliability]
last_updated: 2026-06-18
---

## Summary

Provider configs in filer-core/src/vfs derive Debug and expose secret fields in plaintext. With command-path tracing now in place, those secrets can reach logs. Redact secret fields from Debug output. Found while documenting the secrets boundary in CORE-002.

## Acceptance Criteria

- [ ] Provider configs in filer-core/src/vfs do not print secret values in their Debug output.
- [ ] Secret fields (password, secret_key, session_token, bearer_token, private_key) render as a fixed redaction marker.
- [ ] A regression test proves Debug output of a populated provider config contains no secret value.
