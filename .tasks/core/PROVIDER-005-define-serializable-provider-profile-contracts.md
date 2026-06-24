---
id: PROVIDER-005
title: Define serializable provider profile contracts
status: Done
priority: Medium
type: Feature
parent: PROVIDER-002
rules: [PROVIDER-ACCESS, CORE-LIBRARY]
risk: Medium
impact: "Adds public provider profile and capability contract types."
tags: [provider, vfs]
last_updated: 2026-06-24
---

## Summary

Add provider profile types that serialize portable provider identity without carrying credentials.

## Acceptance Criteria

- [x] Provider profile identifiers, schemes, display names, roots, and capabilities serialize and deserialize.
- [x] Provider profile contracts expose no portable credential, token, password, or secret fields.
- [x] Capabilities are comparable and serializable for provider contract checks.
