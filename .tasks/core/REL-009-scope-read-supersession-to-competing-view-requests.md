---
id: "REL-009"
title: "Scope read supersession to competing view requests"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["CORE-LIBRARY", "SESSION-BOUNDARY", "ACTOR-LONG-WORK"]
risk: "High"
tags: ["cancellation", "preview", "search", "navigation", "bug", "needs-triage"]
whitepaper: "docs/adr/0001-core-runtime-lifecycle.md"
last_updated: "2026-09-05"
---

## Summary

Apply ADR-0001 newest-intent behavior only to reads that replace the same view result. Previewer currently shares its per-Session cancellation slot across preview and metadata work. Inspect existing request classes and test the first interference regression before implementation. Separate request-lifecycle support from actor integration in reviewable stages; reuse current identities and avoid a new view model unless public behavior requires it.

## Acceptance Criteria

- [ ] New navigation, preview, or search intent supersedes prior work for the same purpose and view, with stale results rejected by correlation.
- [ ] Independent metadata loads and directory continuation requests do not cancel one another merely because they belong to the same Session.
- [ ] Explicit cancellation and Session closure still stop the intended read work without cancelling accepted mutations.
- [ ] Public-interface tests cover preview/metadata overlap, query replacement, valid continuation sequencing, late results, and cross-Session isolation using event barriers.
- [ ] cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
