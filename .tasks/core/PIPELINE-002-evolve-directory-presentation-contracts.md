---
id: PIPELINE-002
title: Evolve directory presentation contracts
status: To Do
priority: Medium
type: Epic
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "Changes sorting, grouping, and folder preference behavior across clients."
tags: [pipeline, sorting, grouping, paging, performance]
last_updated: 2026-07-07
---

## Summary

Extend view-independent directory preferences and stable grouping behavior.

This epic also owns the paging scalability ceiling from the CORE-004 audit (finding F24): `load_provider` walks the entire directory to produce one page, even with native provider paging, because only the keyset boundary survives between page requests. The fix spans the pipeline, the provider cursor, and the paging session (a cached materialized order keyed by the cursor, or pipeline-ordered provider paging), which is why it lives here and not in the tactical hot-path task CORE-021.

## Exit Criteria

- [ ] A next-page request costs work proportional to the page, not the directory: no full rewalk of provider entries per page.
- [ ] Folder preferences represent sort, group, hidden-file, and density choices without UI types.
- [ ] Natural and locale-aware comparison modes are explicit.
- [ ] Empty extension, folder, and unknown-type groups have stable labels.
- [ ] Project-aware grouping covers source, config, generated, media, archive, and document categories.
