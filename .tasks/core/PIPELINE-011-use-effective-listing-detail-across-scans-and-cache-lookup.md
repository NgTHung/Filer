---
id: "PIPELINE-011"
title: "Use effective listing detail across scans and cache lookup"
status: "To Do"
priority: "High"
type: "Bug"
parent: "core:CORE-027"
milestone: "0.3.1"
rules: ["PIPELINE-TRANSFORMS", "PROVIDER-ACCESS"]
risk: "High"
tags: ["pipeline", "metadata", "cache", "bug", "ready-for-agent"]
last_updated: "2026-10-02"
---

## Summary

A fast snapshot can filter out a 100-byte file under min_size=50 while the same paged scan returns it. Snapshot and page execution choose listing detail differently. Resolve the metadata required by filters, sorting, and grouping through one owner before provider reads and cache lookup.

## Acceptance Criteria

- [ ] Real LocalFs public scans with fast listing and size or timestamp transforms return equivalent identities and order in bounded snapshots, unbounded snapshots, and collected pages.
- [ ] Execution and cache lookup share effective listing-detail resolution; a fast-only cache entry cannot satisfy a request requiring metadata.
- [ ] Tests cover cache hits and misses, size filters, metadata sorting/grouping, and an unchanged identity-only fast path.
- [ ] cargo fmt --check, cargo check -p filer-core, and cargo test -p filer-core pass.
