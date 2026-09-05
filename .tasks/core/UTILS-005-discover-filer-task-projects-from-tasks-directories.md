---
id: UTILS-005
title: Make filer-task portable across project namespaces
status: Done
priority: High
type: Epic
risk: High
impact: "Changes project discovery, task identity, and validation policy across every filer-task command and library consumer."
tags: [tooling, tasks, cli, portability, namespaces, configuration]
last_updated: 2026-09-05
---

## Summary

Make filer-task reusable across independent projects. Each project is discovered from its nearest `.tasks` directory, treats every domain as an identity namespace, and may configure its allowed prefixes, task types, and tags. `default` is an ordinary domain name with no implicit selection behavior. The refactor prepares filer-task to serve as the library backend for a multi-project web UI, so core operations must work outside the CLI against several explicitly opened projects in one process.

## Exit Criteria

- [x] Commands discover the nearest project from `.tasks`, honor an explicit root, and fail clearly when no project exists.
- [x] `.tasks/default` is a supported domain name with no special resolution behavior, and every CLI task reference states its domain explicitly.
- [x] Task creation accepts a domain-qualified task ID as shorthand for separate domain and local-ID arguments, never infers a domain, and rejects conflicting domain inputs.
- [x] Task IDs are unique within a domain, while separate domains may use the same local ID without validation, lookup, relationship, or lifecycle conflicts.
- [x] Qualified parent and dependency references resolve across domains, and ambiguous or missing references return actionable errors.
- [x] A versioned project configuration customizes domain prefixes, task types, and tag policy without recompiling filer-task.
- [x] Custom task types define the validation behavior that currently distinguishes milestones and container tasks from normal work.
- [x] Projects may keep tags open or enforce a configured tag catalog, with actionable errors for invalid strict-mode tags.
- [x] Library operations accept an explicit project root, keep no working-directory or process-global state, and support several open projects in one process.
- [x] Library errors and results are structured data a programmatic consumer can present without parsing CLI text.
- [x] Existing Filer task repositories remain usable through a documented compatibility and migration path.
- [x] Every behavior, CLI syntax, configuration, output schema, and public library API change updates its corresponding documentation in the same child task so released documentation never describes stale behavior.
- [x] Tests prove isolation across independent projects and across domains that contain identical local task IDs.

## Completion evidence

Reconciled on 2026-09-05. All nine direct children are Done. `cargo test -q -p taskroot` passes, including project_discovery_test, task_domains_test, namespaced_references_test, project_config_test, taxonomy_policy_test, concurrent_project_test, and CLI coverage. These suites cover discovery, explicit namespaces, policy, lifecycle isolation, and independent project access. TaskProject takes an explicit root; TaskError exposes structured codes and context. docs/task-project-contract.md and docs/task-tracking.md describe compatibility, migration, and versioned output. Publication is tracked separately in UTILS-021.
