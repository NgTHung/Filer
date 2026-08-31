---
id: "PIPELINE-005"
title: "Dispatch directory paging on pipeline paging mode"
status: In Progress
priority: "High"
type: "Bug"
parent: "PIPELINE-003"
milestone: "0.3.1"
depends_on: ["PIPELINE-004"]
rules: ["PROVIDER-ACCESS", "PIPELINE-TRANSFORMS", "ACTOR-LONG-WORK"]
risk: "High"
impact: "Changes first-page delivery and continuation cost on the large-directory hot path."
tags: ["core", "paging", "pipeline", "performance", "remediation"]
last_updated: 2026-08-31
---

## Summary

Stage 2 of PIPELINE-003. Consume PipelineConfig::paging_mode in the scanner paging session instead of walking the whole directory for every page. Provider-page and filtered-page modes pull only enough provider rows to fill the requested page and store the continuation handle from PIPELINE-004 in the paging session. Ordered modes keep their current full walk here; PIPELINE-006 makes their continuation proportional.

## Acceptance Criteria

- [ ] A 10,000-entry default local listing emits its first page through public core contracts before the provider reaches end of directory, proven with a controllable provider test.
- [ ] A continuation request in a streaming mode resumes the stored provider walk instead of replaying prior entries, with provider work proportional to the requested page.
- [ ] Sorting, filtering, and grouping configurations keep their current ordering and cursor results unchanged.
- [ ] Paging state composes with the CORE-018 lifetime bound and releases provider continuation resources when the cursor expires, is replaced, or reaches the terminal page.
- [ ] Paging page assembly is split out of the existing paging module so no module exceeds the crate size guidance.
