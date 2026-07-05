---
id: API-003
title: Remove or reject NodeId command routes
status: Obsolete
priority: Medium
type: Refactor
parent: CORE-001
depends_on: [API-002]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Completes the public API migration away from transient NodeId addressing."
tags: [api, compatibility, nodeid, location]
last_updated: 2026-07-05
---

## Summary

After NodeId compatibility APIs are deprecated, either remove those routes from the public command surface or keep them as explicit structured errors. The decision must be deliberate and tested.

## Rationale

Merged into API-004. The project direction is complete NodeId removal, so retaining this separate remove-or-reject compatibility task would preserve a rejected option.
