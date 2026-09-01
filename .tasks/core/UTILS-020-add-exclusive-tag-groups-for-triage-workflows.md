---
id: "UTILS-020"
title: "Add exclusive tag groups for triage workflows"
status: In Progress
priority: "High"
type: "Feature"
risk: "Medium"
impact: "Adds validated triage classification and atomic tag transitions to filer-task projects and agent workflows."
tags: ["tasks", "workflow", "cli", "validation"]
last_updated: 2026-09-01
---

## Summary

Let projects define mutually exclusive tag groups and let callers change one group atomically, so triage labels remain queryable task metadata without duplicating lifecycle status or parsing Markdown bodies.

## Acceptance Criteria

- [ ] Project configuration can define optional exclusive tag groups whose values belong to the allowed tag catalog.
- [ ] Task validation rejects multiple tags from one exclusive group without affecting projects that define no groups.
- [ ] A public library operation and CLI command set or clear one group value atomically while preserving unrelated tags and original bytes on failure.
- [ ] Existing list and ready tag filters select triage roles while structural readiness rules remain unchanged.
- [ ] Filer declares triage category and state groups, and task-tracking documentation explains their use.
