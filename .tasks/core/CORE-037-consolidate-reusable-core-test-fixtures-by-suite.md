---
id: "CORE-037"
title: "Consolidate reusable core test fixtures by suite"
status: Done
priority: "Medium"
type: "TestDebt"
parent: "core:CORE-022"
milestone: "0.3.1"
risk: "Low"
tags: ["core", "testing", "enhancement", "ready-for-agent"]
last_updated: 2026-09-18
---

## Summary

Build on tests/support/mod.rs. Inventory node builders and provider doubles in tests/ and src/tests/, then migrate top-level scanner, search, and navigation integration fixtures first. Migrate equivalent internal fixtures one cluster per commit using the same reusable setup where their harness permits it. Keep specialized timeout, paging, or watch doubles explicit; document why each remaining variant needs distinct behavior. Record a pre-change test inventory so consolidation cannot remove coverage.

## Acceptance Criteria

- [x] Reusable NodeEntry construction has one shared implementation accessible to the relevant test harnesses, replacing equivalent make_file variants.
- [x] Equivalent provider setup uses shared configurable support; remaining specialized doubles and their behavioral differences are recorded.
- [x] Each commit migrates one test cluster within repository diff guidance, preserves assertions and test coverage, and passes that cluster.
- [x] The full filer-core test suite passes after consolidation.

## Execution Plan

See [fixture inventory and staged plan](../../docs/core-037-fixture-consolidation.md).

## Validation

Default, minimal-feature, and all-feature filer-core suites pass. Default and
all-feature Clippy pass with warnings denied. The pre-change test inventory and
assertion counts are preserved. Each suite migration has its own tested commit;
see the execution plan for counts, commit IDs, and retained provider differences.
