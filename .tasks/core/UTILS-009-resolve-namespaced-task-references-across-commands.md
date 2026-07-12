---
id: UTILS-009
title: Resolve namespaced task references across commands
status: To Do
priority: High
type: Refactor
parent: UTILS-005
depends_on: [UTILS-008]
risk: High
impact: "Changes lookup and relationship semantics across lifecycle, readiness, context, dependency, milestone, and output commands."
tags: [tooling, tasks, cli, namespaces]
last_updated: 2026-07-12
---

## Summary

Apply the namespace contract to every task reference. CLI lookups, stored relationships, graph validation, and lifecycle mutations must target one domain-qualified task without collisions.

## Acceptance Criteria

- [ ] Every CLI lookup or mutation command that accepts a task ID requires the canonical domain-qualified identity and rejects unqualified references with an error listing the matching qualified candidates.
- [ ] Task creation accepts the canonical domain-qualified identity as shorthand for separate domain and local-ID arguments, never infers a domain, and rejects conflicting inputs.
- [ ] Parent and dependency references resolve within and across domains according to the namespace contract.
- [ ] Duplicate detection, cycle detection, ancestors, children, dependents, blockers, readiness, and milestone views key tasks by domain plus local ID.
- [ ] Lifecycle commands mutate only the selected domain when another domain contains the same local ID.
- [ ] Missing, malformed, and ambiguous references fail with actionable errors that identify the relevant namespace.
- [ ] Human and JSON output expose enough canonical identity information to address every returned task without ambiguity, with schema changes documented.
- [ ] End-to-end tests cover same-domain and cross-domain relationships, duplicate local IDs, every ID-taking command family, and compatibility with the existing Filer task tree.
- [ ] CLI help, examples, task-tracking documentation, and human and JSON output-schema documentation explain mandatory CLI qualification, same-domain frontmatter references, cross-domain relationships, and migration from globally unique IDs.
