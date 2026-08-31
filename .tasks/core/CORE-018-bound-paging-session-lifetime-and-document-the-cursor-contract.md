---
id: CORE-018
title: Bound paging session lifetime and document the cursor contract
status: In Progress
priority: Medium
type: Bug
parent: CORE-027
milestone: "0.3.1"
depends_on: [CORE-015]
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "Abandoned pagination grows memory without limit and the cursor's mutation behavior is undocumented."
tags: [core, audit, remediation, pipeline]
last_updated: 2026-08-31
---

## Summary

The paging session map has no TTL or cap. A session is inserted on every page that has more and removed only on continuation or a fresh cursorless load, so a client that loads page one and never continues leaves a full FileNode clone plus a cloned PipelineConfig resident until that owner starts another listing or ends. Across abandoned scrolls the map grows unbounded. Bound it with a hard 256-session LRU so the resident continuation state has a deterministic ceiling. Separately, document the cursor contract: the keyset row is a point-in-time snapshot, so a metadata sort under concurrent mutation can duplicate or skip a row, total_count is a first-page estimate not a running count, and the cursor is currently single-use. Replay tolerance is out of scope because retaining consumed continuation state would weaken the memory bound and PIPELINE-003 will change the stored provider continuation shape; clients recover with a cursorless refresh. Depends on CORE-015 because the boundary guarantee rests on a single comparator.

This task owns session map lifetime and cursor documentation only. PIPELINE-003 depends on this task and owns first-page streaming and O(directory) next-page cost under native paging (audit F24).

Milestone 0.3.1 (MILESTONE-004 draft). Tracked under CORE-027 post-audit remediation.

## Acceptance Criteria

- [ ] The paging session map retains at most 256 continuation sessions, evicts the oldest unconsumed cursor when full, and has a regression test for eviction and expired-cursor recovery.
- [ ] Public cursor documentation explains the keyset snapshot semantics, mutation caveat, first-page total estimate, bounded lifetime, and single-use behavior.
- [ ] Replay tolerance is explicitly out of scope because consumed state is not retained; the documentation names cursorless refresh as the recovery path and explains why PIPELINE-003 may revisit the decision.
