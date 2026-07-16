---
id: "WEB-021"
title: "Serve project policy and validate strict filter references"
status: Done
priority: "High"
type: "Feature"
parent: "WEB-001"
risk: "Medium"
impact: "Exposes the project policy catalog the New-task form and strict-mode tag filtering need, and makes the list endpoint reject invalid strict-mode filter references instead of returning empty results."
tags: ["web", "tasks", "api", "configuration"]
last_updated: 2026-07-16
---

## Summary

The frontend has no way to learn a project's policy: the New-task form needs each domain's allowed prefixes, the task types, and the tag catalog for its pickers, and the Tasks screen's tag filter must offer only catalog tags when the policy is strict. Add GET /api/projects/{project}/policy returning a read-only view of the project config: domains with their prefixes, task types, and the tag policy with its catalog. Also validate filter references on GET /api/projects/{project}/tasks: under a strict tag policy an unknown tag returns the structured tag_rejected error instead of an empty list, and a bare parent id that matches more than one domain is rejected through filer-task's existing reference resolution, reusing its error code and context rather than reinventing them.

## Acceptance Criteria

- [x] GET /api/projects/{project}/policy returns the project's domains with prefixes, task types, and tag policy with catalog, read-only.
- [x] GET /api/projects/{project}/tasks with an unknown tag under strict policy returns the structured tag_rejected error; under open policy the filter applies and may return an empty list.
- [x] A bare parent filter id matching more than one domain is rejected with filer-task's existing ambiguous-reference error and its candidate context.
- [x] In-process tests cover the policy shape, strict and open tag filtering, and the ambiguous parent rejection, following the existing oneshot test pattern.
