---
id: "SEARCH-001"
title: "Expand provider-aware search"
status: "To Do"
priority: "Medium"
type: "Epic"
milestone: "0.5.0"
rules: ["PROVIDER-ACCESS", "ACTOR-LONG-WORK", "SESSION-BOUNDARY"]
risk: "Medium"
impact: "Extends search roots, indexing, and provider delegation."
tags: ["search", "provider", "indexing", "enhancement", "needs-triage"]
last_updated: "2026-09-05"
---

## Summary

Add scoped roots, optional indexing, and native provider search behind core-owned routing.

Core search expansion does not hard-depend on MODULES-001. Extension search contributions wait until the wire-safe extension data plane exists.

SEARCH-002 specifies explicit roots first and creates bounded routing and regression tickets. Native provider delegation follows; indexing needs a measured workload before implementation is selected. Extension contributions become a separate stage depending on the host contracts. Preserve the remaining outcomes below when refining each stage.

## Exit Criteria

- [ ] Search roots can target selected folders, the current folder, or a workspace.
- [ ] Large projects can use an indexed search service without changing result contracts.
- [ ] Providers can advertise and receive native search delegation.
- [ ] When MODULES-001 exists, extension search contributions preserve core cancellation, request identity, and event routing.
