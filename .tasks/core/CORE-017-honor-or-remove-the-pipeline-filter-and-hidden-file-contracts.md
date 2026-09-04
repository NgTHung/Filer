---
id: CORE-017
title: Honor or remove the pipeline filter and hidden-file contracts
status: In Progress
priority: Medium
type: Bug
parent: CORE-027
milestone: "0.3.1"
rules: [PIPELINE-TRANSFORMS]
risk: Medium
impact: "The pipeline charges for filters it never applies and hides the wrong files on Windows."
tags: [core, audit, remediation, pipeline]
last_updated: 2026-09-04
---

## Summary

Two pipeline filter contracts lie. First, FilterConfig::min_size/max_size/name_pattern are accepted and force SnapshotOnly plus full-metadata listing, then no stage ever applies them, so a request pays the cost and returns unfiltered rows. Either implement the three stages or remove the fields and their paging_mode/effective_listing influence; SearchQuery already implements this logic and should be the shared predicate source. Second, FilterHidden recomputes hidden state from the path (leading-dot only) and is wrong on Windows, where FILE_ATTRIBUTE_HIDDEN files are not dotfiles, diverging from the platform-correct meta.hidden that QueryFilter::IsHidden uses. Filter on meta.hidden so listing and search share one definition.

Milestone 0.3.1 (MILESTONE-004 draft). Tracked under CORE-027 post-audit remediation.

## Acceptance Criteria

- [ ] FilterConfig size and name filters either apply as real stages or are removed along with their paging_mode and effective_listing influence; no accepted config is silently ignored.
- [ ] If implemented, the size and name predicates are shared with SearchQuery rather than reimplemented.
- [ ] FilterHidden filters on NodeMeta::hidden so the Windows hidden-attribute case is honored, pinned by a test that distinguishes a dotfile from a hidden-attribute file.
