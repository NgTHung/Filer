---
id: SEARCH-001
title: Expand provider-aware search
status: To Do
priority: Medium
type: Epic
depends_on: [MODULES-001]
rules: [PROVIDER-ACCESS, ACTOR-LONG-WORK, SESSION-BOUNDARY]
risk: Medium
impact: "Extends search roots, indexing, and provider delegation."
tags: [search, provider, indexing]
last_updated: 2026-06-06
---

## Summary

Add scoped roots, optional indexing, and native provider search behind core-owned routing.

## Exit Criteria

- [ ] Search roots can target selected folders, the current folder, or a workspace.
- [ ] Large projects can use an indexed search service without changing result contracts.
- [ ] Providers can advertise and receive native search delegation.
- [ ] Extension search contributions preserve core cancellation, request identity, and event routing.
