---
id: API-003
title: Remove or reject NodeId command routes
status: To Do
priority: Medium
type: Refactor
parent: CORE-001
depends_on: [API-002]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Completes the public API migration away from transient NodeId addressing."
tags: [api, compatibility, nodeid, location]
last_updated: 2026-06-24
---

## Summary

After NodeId compatibility APIs are deprecated, either remove those routes from the public command surface or keep them as explicit structured errors. The decision must be deliberate and tested.

## Acceptance Criteria

- [ ] A compatibility decision is documented in the task or code docs: remove NodeId routes entirely, or keep them as structured UnsupportedOperation or InvalidLocation errors.
- [ ] Command routing no longer performs provider work from NodeId-only inputs.
- [ ] Tests prove deprecated NodeId command routes are absent or return the chosen structured error.
- [ ] LocationRef command routes continue to cover navigation, scan, preview, metadata, search, watch, and operations.
