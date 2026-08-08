---
id: "WEB-023"
title: "Define a versioned task-progress publish contract"
status: "To Do"
priority: "High"
type: "Feature"
parent: "WEB-022"
risk: "Low"
impact: "Gives independent services a stable contract for task progress without exposing Filer files or internal serializers."
tags: ["web", "api", "contracts", "sync"]
last_updated: "2026-08-08"
---

## Summary

Define protocol version 1 from docs/superpowers/specs/2026-08-08-external-task-progress-publish-contract.md in `filer-task-web`. Add dedicated outbound DTOs for project identity, normalized tasks, criteria, milestone aggregates, source metadata, and the success and error envelopes. Build them only from a validated project. Use closed lowercase machine values for status and priority while preserving configured task type names. Sort every order-independent collection and compute the stable content hash described by the contract. Keep these DTOs separate from existing HTTP read views so either surface can evolve without silently changing the other. Pin the wire shape with JSON fixtures that an independent JavaScript project can consume.

## Acceptance Criteria

- [ ] Protocol version 1 serializes normalized project, task, criterion, source, and milestone progress using the documented machine values and nullability.
- [ ] The payload contains no local paths, raw Markdown, arbitrary document sections, or Rust-specific response wrappers.
- [ ] Snapshot construction refuses validation errors, preserves criterion order, and sorts every order-independent collection.
- [ ] The content hash is stable when progress is unchanged, excludes generation time, and changes when published progress or source metadata changes.
- [ ] Checked-in JSON fixtures and tests pin request serialization, success and error parsing, response-hash verification, and retry classification for external consumers.
