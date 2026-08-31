---
id: PIPELINE-002
title: Evolve directory presentation contracts
status: To Do
priority: Medium
type: Epic
milestone: "0.5.0"
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "Changes sorting, grouping, and folder preference behavior across clients."
tags: [pipeline, sorting, grouping, performance]
last_updated: 2026-08-31
---

## Summary

Extend view-independent directory preferences and stable grouping behavior.

PIPELINE-003 owns first-page streaming and proportional continuation cost. This epic may build presentation behavior on that paging contract, but it does not own provider continuation or paging session state.

Milestone 0.5.0 (MILESTONE-006 draft). Cross-provider paging contracts already landed in PIPELINE-001.

## Exit Criteria

- [ ] Folder preferences represent sort, group, hidden-file, and density choices without UI types.
- [ ] Natural and locale-aware comparison modes are explicit.
- [ ] Empty extension, folder, and unknown-type groups have stable labels.
- [ ] Project-aware grouping covers source, config, generated, media, archive, and document categories.
