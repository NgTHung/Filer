---
id: "CORE-037"
title: "Consolidate reusable core test fixtures by suite"
status: "To Do"
priority: "Medium"
type: "TestDebt"
parent: "core:CORE-022"
milestone: "0.3.1"
risk: "Low"
tags: ["core", "testing"]
last_updated: "2026-09-05"
---

## Summary

Build on tests/support/mod.rs. Inventory node builders and provider doubles in tests/ and src/tests/, then migrate top-level scanner, search, and navigation integration fixtures first. Migrate equivalent internal fixtures one cluster per commit using the same reusable setup where their harness permits it. Keep specialized timeout, paging, or watch doubles explicit; document why each remaining variant needs distinct behavior. Record a pre-change test inventory so consolidation cannot remove coverage.

## Acceptance Criteria

- [ ] Reusable NodeEntry construction has one shared implementation accessible to the relevant test harnesses, replacing equivalent make_file variants.
- [ ] Equivalent provider setup uses shared configurable support; remaining specialized doubles and their behavioral differences are recorded.
- [ ] Each commit migrates one test cluster within repository diff guidance, preserves assertions and test coverage, and passes that cluster.
- [ ] The full filer-core test suite passes after consolidation.
