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
last_updated: 2026-09-05
---

## Summary

Extend view-independent directory preferences and stable grouping behavior.

PIPELINE-003 owns first-page streaming and proportional continuation cost. This epic may build presentation behavior on that paging contract, but it does not own provider continuation or paging session state.

PIPELINE-007 specifies preference ownership and the first application slice, then creates bounded implementation stages. Comparison modes and project grouping follow as separate stages on the completed PIPELINE-003 paging contract.

## Exit Criteria

- [ ] Core applies per-folder sort, group, and hidden-file preferences through PipelineConfig; clients retain visual density and UI persistence ownership.
- [ ] Natural and locale-aware comparison modes are explicit.
- [ ] Empty extension, folder, and unknown-type groups have stable labels.
- [ ] Project-aware grouping covers source, config, generated, media, archive, and document categories.
