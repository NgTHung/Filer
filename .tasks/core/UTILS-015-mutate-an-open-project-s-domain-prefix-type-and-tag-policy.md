---
id: UTILS-015
title: Mutate an open project's domain, prefix, type, and tag policy
status: Done
priority: High
type: Feature
parent: UTILS-013
depends_on: [UTILS-011]
risk: High
impact: "Lets a project's config.json grow (new domain, prefix, task type, or catalog tag) without hand-editing the file, while guaranteeing the change never invalidates an already-stored task."
tags: [tasks, library, configuration, validation]
last_updated: 2026-07-14
---

## Summary

ProjectPolicy (project.rs) is read-only after TaskProject::open — core:UTILS-011 built the enforcement side (prefixes, types, strict tags) but nothing can add to the catalog at runtime. Add policy-mutation functions (add_domain, add_prefix, add_task_type, add_tag / remove_tag) that re-validate every existing task file against the proposed policy before writing config.json, and reject the change with the same structured error shape validate_repo already uses if any task would become invalid (for example removing a tag or prefix still in use).

## Acceptance Criteria

- [x] Adding a domain, prefix, task type, or tag writes config.json only after confirming validate_repo still passes for every existing task under the proposed policy.
- [x] Removing a prefix, task type, or tag still referenced by an existing task is rejected with a structured error naming the blocking task, and config.json is left unchanged.
- [x] Mutations use the same atomic-write guarantee as task file writes — a failed mutation leaves the previous config.json intact.
- [x] Tests cover a successful additive change, a rejected removal blocked by an existing task, and config.json being byte-identical to its prior state after a rejected mutation.
