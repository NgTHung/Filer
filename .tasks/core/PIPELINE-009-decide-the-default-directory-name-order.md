---
id: "PIPELINE-009"
title: "Decide the default directory name order"
status: "To Do"
priority: "High"
type: "Design"
milestone: "0.3.1"
rules: ["PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Changes the order that every sorted listing and ordered continuation uses."
tags: ["pipeline", "sorting", "paging", "bug", "ready-for-agent"]
last_updated: "2026-09-24"
---

## Summary

Name sort compares raw bytes in filer-core/src/pipeline/order.rs, so "Zeta" sorts before "alpha" and "file10" sorts before "file2". Explorer-style clients expect case-insensitive, number-aware order. The same comparator defines ordered continuation order, so decide the default before PIPELINE-008 optimizes the sorted path. Record the order, its tie-breakers, and its cost. User-selectable and locale-aware modes stay with PIPELINE-002.

## Acceptance Criteria

- [ ] An ADR under docs/adr/ records the default Name order, covering case handling, digit runs, leading zeros, non-ASCII names, and the tie-breakers that keep the order total.
- [ ] Example pairs in the ADR show the expected order for mixed case, numbered names, leading zeros, and non-ASCII names.
- [ ] The ADR states how ordered continuations and cursors stay stable under the chosen order, and whether each row derives its sort key once instead of per comparison.
- [ ] The ADR reports the chosen comparator's sort time on 10,000 generated names next to the current byte comparison on one named machine.
- [ ] The ADR assigns implementation to PIPELINE-008 or a named follow-up task, leaves user-selectable and locale-aware modes with PIPELINE-002, and the maintainer accepts it.
