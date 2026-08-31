---
id: "PIPELINE-003"
title: "Make large-directory paging proportional to page size"
status: Done
priority: "High"
type: "Bug"
parent: "CORE-027"
milestone: "0.3.1"
depends_on: ["PIPELINE-001", "CORE-018"]
rules: ["PROVIDER-ACCESS", "PIPELINE-TRANSFORMS", "ACTOR-LONG-WORK"]
risk: "High"
impact: "Changes provider traversal, paging session state, and first-page delivery on the large-directory hot path."
tags: ["core", "audit", "remediation", "pipeline", "paging", "performance"]
last_updated: 2026-08-31
---

## Summary

Own the CORE-004 F24 scalability fix split from PIPELINE-002. For paging modes that do not require a full snapshot, preserve provider and pipeline continuation state so the first page can arrive before the directory walk completes and later pages do not restart that walk. Snapshot-only transforms must remain explicit and correct. CORE-018 owns session lifetime and cursor documentation; CORE-021 owns per-row allocation; CORE-028 measures the resulting public-contract behavior.

The work is staged across three children. PIPELINE-004 adds the provider continuation handle, PIPELINE-005 dispatches page assembly on the pipeline paging mode, and PIPELINE-006 makes ordered continuations proportional and documents the streaming total-count contract. The criteria below stay here as the outcome this task owns.

## Acceptance Criteria

- [x] A 10,000-entry default local listing emits its first page through public core contracts before the provider reaches end of directory, proven with a controllable provider test.
- [x] A continuation request resumes stored provider and pipeline progress instead of replaying prior entries or walking the full directory again, with work proportional to the requested page under stable input.
- [x] Configurations that require a complete snapshot remain explicit and preserve their sorting, filtering, grouping, and cursor correctness instead of claiming streaming behavior.
- [x] Paging state composes with the CORE-018 lifetime bound and releases provider continuation resources when the cursor expires, is replaced, or reaches the terminal page.
- [x] The public paging path exposes enough observable behavior for CORE-028 to measure first-page and next-page latency without private benchmark hooks.
