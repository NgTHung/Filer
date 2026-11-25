---
id: "API-021"
title: "Add a direct LocationRef constructor for local paths"
status: "To Do"
priority: "Low"
type: "Feature"
milestone: "0.3.1"
risk: "Low"
impact: "Adds public constructors to LocationRef without changing existing variants or identities."
tags: ["api", "location", "model", "ready-for-agent"]
last_updated: "2026-11-25"
---

## Summary

Building a LocationRef for a local path takes two calls, LocationRef::from_location(&Location::local(path)), and the pattern appears about 160 times across filer-core sources, tests, benches, and examples. Clients such as the terminal client need it to address a start directory. Add LocationRef::local(path) and From<&Location> for LocationRef so callers build the Full variant in one step. Migrating existing call sites is a separate mechanical change.

## Acceptance Criteria

- [ ] LocationRef::local(path) returns a value equal to LocationRef::from_location(&Location::local(path)), including the same identity, proven by a public-interface test written before the implementation.
- [ ] From<&Location> for LocationRef produces the Full variant equal to LocationRef::from_location, proven by a test.
- [ ] Both constructors have rustdoc with a runnable example, and the change touches no existing call sites.
- [ ] cargo fmt --check, cargo clippy -p filer-core, and cargo test -p filer-core pass.

## Rationale

A missing constructor repeated at about 160 call sites is a type gap. Fixing the type first keeps the later call-site migration mechanical and reviewable.

