---
id: REL-001
title: Complete structured error context
status: To Do
priority: High
type: Feature
parent: CORE-001
milestone: "0.3.0"
rules: [PROVIDER-ACCESS]
risk: Medium
impact: "Changes error context consumed by app and transport clients."
tags: [reliability, errors, provider]
last_updated: 2026-06-06
---

## Summary

Add stable context for collisions, stale requests, and provider capability failures.

## Acceptance Criteria

- [ ] Collision errors identify the conflicting source and destination context.
- [ ] Stale-request errors identify session and request identity.
- [ ] Provider capability errors identify the provider, location, and missing capability.
- [ ] Clients can branch on structured fields without parsing messages.
