---
id: UTILS-003
title: Trim redundant fields from filer-task JSON output
status: Done
priority: Low
type: Refactor
risk: Low
impact: "Reduces token cost of agent-facing JSON without changing the schema contract."
tags: [cli, output, json]
last_updated: 2026-06-13
---

## Summary

Agent-facing JSON has two avoidable costs. The context command emits acceptance criteria twice: as raw markdown inside sections[].content and again as the structured criteria[] array. List-style output (list, ready, related) repeats null and empty fields (parent, milestone, rules, whitepaper) on every task. Keep criteria[] as the single structured source of truth and let sections carry only free-form prose, and skip serializing null/empty optional fields. schema_version stays at 1 because absent fields remain backward-compatible with parsers that treat them as null or empty.

## Acceptance Criteria

- [x] context JSON no longer repeats acceptance-criteria text inside sections[].content.
- [x] context JSON still exposes criteria through the structured criteria[] array.
- [x] Null optional fields and empty arrays are omitted from task JSON in list, ready, context, and related output.
- [x] schema_version remains 1 and existing JSON consumers parse the trimmed output.
- [x] output and cli tests assert the absence of duplicated criteria and omitted empty fields.
