---
id: PROVIDER-006
title: Add runtime provider registry resolution
status: Done
priority: Medium
type: Feature
parent: PROVIDER-002
depends_on: [PROVIDER-005]
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: High
impact: "Introduces runtime provider lookup by local, profile, and ephemeral provider references."
tags: [provider, vfs]
last_updated: 2026-06-24
---

## Summary

Add a core-owned runtime provider registry that resolves provider references to live providers and capability contracts.

## Acceptance Criteria

- [x] Local, profile, and ephemeral provider references resolve through one registry API.
- [x] Profile registration validates the profile scheme against the live provider scheme.
- [x] Unknown profile and ephemeral references return structured unsupported-provider errors.
