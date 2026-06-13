---
id: CORE-012
title: Review documentation and comment-rule compliance
status: To Do
priority: Low
type: Docs
parent: CORE-004
risk: Low
impact: "Docs that drift from code mislead every future contributor and frontend author."
tags: [core, audit, docs]
last_updated: 2026-06-13
---

## Summary

Review module docs against the WHY-not-WHAT rule, comment-rule compliance, and the accuracy of README/DESIGN against the current code.

## Acceptance Criteria

- [ ] Report at docs/reviews/filer-core/documentation.md flags comment-rule violations with file:line.
- [ ] README and DESIGN claims that no longer match the code are listed.
- [ ] Follow-up doc-fix candidates are listed.
