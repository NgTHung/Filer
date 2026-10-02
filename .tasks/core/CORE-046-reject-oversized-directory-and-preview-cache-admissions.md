---
id: "CORE-046"
title: "Reject oversized directory and preview cache admissions"
status: "To Do"
priority: "Medium"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["CORE-LIBRARY"]
risk: "Medium"
tags: ["cache", "bug", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

An entry larger than the whole cache budget is retained after eviction empties the cache. Apply one shared admission rule where practical so oversized directory and preview results remain usable without being cached. This task bounds accounted retained bytes, not transient scan allocations.

## Acceptance Criteria

- [ ] Directory and preview caches retain no entry above their configured accounted-byte budget, including a zero budget.
- [ ] Oversized results still reach the caller; replacement, eviction, and rejected replacement leave accounting and existing entries consistent.
- [ ] Tests cover zero capacity, oversized insertion, replacement, and normal fitting entries in both caches through their public interfaces.
- [ ] cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
