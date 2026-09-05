---
id: "CORE-021"
title: "Cut pipeline hot-path allocation on the large-directory scan"
status: In Progress
priority: "High"
type: "Refactor"
parent: "CORE-027"
milestone: "0.3.1"
rules: ["PIPELINE-TRANSFORMS"]
risk: "Low"
impact: "Per-row allocation and dead stage work on the scan hot path tax the performance-first priority."
tags: ["core", "audit", "remediation", "performance", "enhancement", "ready-for-agent"]
last_updated: 2026-09-05
---

## Summary

Grouped filters already retain NodeEntry values in place in pipeline/filter.rs, and grouped sorting uses mem::take in pipeline/sort.rs. Preserve those improvements. Both modules/scan/paging/stream.rs and modules/scan/paging/selection.rs still call execute_flat(vec![entry]) for each provider row. Remove those temporary vectors and selection-irrelevant stages through shared Pipeline-owned filtering logic. Keep paging mechanics outside the predicate implementation.

CORE-018 and PIPELINE-003 are Done. Preserve their bounded cursor lifetime, streaming lookahead, ordered continuation, and cancellation contracts. Comparator changes, new filter semantics, and scanner decomposition are outside this task.

Use the existing pipeline_filter_contract_test and scanner paging tests as behavioral references. Measure the existing path before implementation and repeat on the same generated input after the change.

## Acceptance Criteria

- [x] Grouped filters retain nodes in place and grouped sorting moves its input without cloning rows.
- [x] Streaming page assembly and PageSelection share Pipeline-owned row filtering without allocating a temporary Vec or executing sort/group stages for each row.
- [x] Regression tests compare flat and paged results for hidden, extension inclusion/exclusion, name, and size filters; sorted and grouped output, lookahead, and cancellation behavior remain correct.
- [x] Before/after allocation counts and allocated bytes on a 10,000-entry fixture demonstrate the reduction for both paging paths; public first/next-page timings are recorded with the machine and revision.
