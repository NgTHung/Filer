---
id: CORE-018
title: Bound paging session lifetime and document the cursor contract
status: To Do
priority: Medium
type: Bug
parent: CORE-004
depends_on: [CORE-015]
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "Abandoned pagination grows memory without limit and the cursor's mutation behavior is undocumented."
tags: [core, audit, remediation, pipeline]
last_updated: 2026-06-13
---

## Summary

The paging session map has no TTL and no cap. A session is inserted on every page that has more and removed only on continuation or a fresh cursorless load, so a client that loads page one and never continues leaves a full FileNode clone plus a cloned PipelineConfig resident until that owner starts another listing or ends. Across abandoned scrolls the map grows unbounded. Bound it with a TTL or LRU. Separately, document the cursor contract: the keyset row is a point-in-time snapshot, so a metadata sort under concurrent mutation can duplicate or skip a row, total_count is a first-page estimate not a running count, and the cursor is currently single-use. Consider making cursors replay-tolerant. Depends on CORE-015 because the boundary guarantee rests on a single comparator.

## Acceptance Criteria

- [ ] The paging session map is bounded by a TTL or LRU so abandoned pagination cannot grow memory without limit, pinned by a test.
- [ ] The cursor contract documents the keyset snapshot semantics, the first-page total estimate, and the single-use behavior.
- [ ] Replay tolerance is either implemented or explicitly documented as out of scope with the reason.
