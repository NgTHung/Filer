---
id: "PIPELINE-007"
title: "Specify stable presentation preferences on paged directories"
status: "To Do"
priority: "Medium"
type: "Design"
parent: "core:PIPELINE-002"
milestone: "0.5.0"
depends_on: ["milestones:MILESTONE-005", "core:PIPELINE-003"]
rules: ["PIPELINE-TRANSFORMS", "CORE-LIBRARY"]
risk: "Medium"
tags: ["core", "enhancement", "ready-for-agent"]
last_updated: "2026-09-05"
---

## Summary

Define the first PIPELINE-002 slice: apply per-folder sort/group/hidden preferences through PipelineConfig while preserving honest paging modes. Keep visual density and durable UI storage with client/profile owners. Specify stable group labels and later natural/locale comparison stages without inventing another comparator authority.

## Acceptance Criteria

- [ ] The design identifies preference ownership, runtime application, invalidation, and the transition between streaming and snapshot-only configurations.
- [ ] Behavioral examples cover default preferences, overrides, group labels, and pagination without leaking UI types into core.
- [ ] Implementation tickets split preference application, comparison modes, and project grouping into bounded stages with shared Pipeline ownership.
