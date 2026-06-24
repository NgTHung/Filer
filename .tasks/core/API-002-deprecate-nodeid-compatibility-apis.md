---
id: API-002
title: Deprecate NodeId compatibility APIs
status: To Do
priority: High
type: Refactor
parent: CORE-001
milestone: "0.3.0"
depends_on: [CORE-025]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: "Changes the public migration path for clients still using NodeId command and event surfaces."
tags: [api, compatibility, nodeid, location]
last_updated: 2026-06-24
---

## Summary

Mark NodeId command and event surfaces as compatibility-only and deprecated once actor internals are LocationRef-native.

## Acceptance Criteria

- [ ] Rustdoc for NodeId command and event variants clearly marks them deprecated compatibility surfaces and points clients to LocationRef variants.
- [ ] Wire command naming keeps compatibility variants explicit so clients can distinguish deprecated NodeId routes from LocationRef routes.
- [ ] Tests or API snapshots prove LocationRef variants are preferred and NodeId variants still translate correctly during the deprecation window.
- [ ] The migration notes state that NodeId is not stable addressing for provider, segmented, archive, virtual, or remote locations.
