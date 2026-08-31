---
id: "PIPELINE-006"
title: "Serve ordered directory pages from a retained snapshot"
status: Done
priority: "High"
type: "Bug"
parent: "PIPELINE-003"
milestone: "0.3.1"
depends_on: ["PIPELINE-005"]
rules: ["PIPELINE-TRANSFORMS"]
risk: "Medium"
impact: "Changes memory held per paging session and the reported total count on streaming pages."
tags: ["core", "paging", "pipeline", "performance", "docs"]
last_updated: 2026-08-31
---

## Summary

Stage 3 of PIPELINE-003. Sorted and grouped views need the full directory before the first correct page, so they keep that walk. Retain the resulting ordered rows in the paging session and serve later pages from it instead of walking again. Bound what a session may retain so the CORE-018 capacity stays a real memory bound. Document the streaming total-count contract that PIPELINE-005 introduces.

## Acceptance Criteria

- [x] A continuation in an ordered or snapshot-only mode serves its page from retained state with work proportional to the requested page, not to the directory.
- [x] Configurations that require a complete snapshot remain explicit and preserve their sorting, filtering, grouping, and cursor correctness instead of claiming streaming behavior.
- [x] Retained paging state is bounded so the session capacity limits memory, not just session count.
- [x] The cursor contract documents when total count is unknown on a streaming page and when it becomes known.
- [x] The public paging path exposes enough observable behavior for CORE-028 to measure first-page and next-page latency without private benchmark hooks.
