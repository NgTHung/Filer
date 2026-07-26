---
id: "WEB-023"
title: "Build a publishable task snapshot bundle in filer-task"
status: "To Do"
priority: "High"
type: "Feature"
parent: "WEB-022"
risk: "Low"
impact: "Defines the wire format every later mirror stage depends on, and makes publishing possible by shell pipeline before any server work lands."
tags: ["tasks", "cli", "contracts", "sync"]
last_updated: "2026-07-26"
---

## Summary

The mirror needs the raw contents of a task repository, not parsed records, so it can materialize a directory that behaves like an ordinary checkout. Add a bundle module to filer-task that builds a TaskBundle carrying a format version, project name, generation timestamp, an optional source describing branch and commit for display, a content hash, and a sorted list of relative path and contents pairs. Ship config.json inside the bundle so a consumer enforces the same policy as the source project. The builder runs validate_repo first and returns an error instead of a bundle when the repository has validation errors, so a broken checkout can never overwrite a good mirror. Compute the content hash as SHA-256 over the sorted path and content pairs using the sha2 dependency the crate already carries for criterion hashing, which gives publishing a cheap idempotency check. Add a bundle command that writes the result to a file or stdout. The command does not speak HTTP, so filer-task gains no client dependency and publishing without a server stays a shell pipeline.

## Acceptance Criteria

- [ ] A bundle carries every .tasks/ file including config.json as raw contents keyed by repository-relative path.
- [ ] The builder refuses to produce a bundle for a repository with validation errors.
- [ ] The content hash is stable across file ordering and changes when any file content changes.
- [ ] The bundle command writes to a file or stdout and filer-task gains no HTTP dependency.
- [ ] Tests cover serialization round-trip, hash stability, and refusal on a broken repository.
