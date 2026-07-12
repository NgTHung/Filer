---
id: UTILS-006
title: Define project configuration and namespace contracts
status: Done
priority: High
type: Design
parent: UTILS-005
risk: High
impact: "Defines task identity, configurable validation policy, and compatibility rules consumed by every command."
tags: [tooling, tasks, namespaces, configuration]
whitepaper: docs/task-project-contract.md
last_updated: 2026-07-12
---

## Summary

Specify the portable project contract before changing stored references and validation. The design must cover namespace identity and a versioned `.tasks/config.json` for domains, domain prefixes, task types, tag policy, and existing Filer repositories. It must also define the library API boundary so a non-CLI consumer, such as a multi-project web UI, can drive every operation.

## Acceptance Criteria

- [x] The design defines a canonical qualified task identity syntax for CLI arguments, frontmatter references, human output, and JSON output.
- [x] The design requires an explicit domain in every CLI task reference, restricts unqualified frontmatter references to the containing domain, and defines every rejection and missing-reference error.
- [x] The design defines how task creation accepts a qualified ID as shorthand for separate domain and local-ID arguments, requires an explicit domain, and rejects conflicting domain inputs.
- [x] Task identity is specified as domain plus local ID, with local IDs unique only within their domain.
- [x] Valid domain names, milestone handling, and reserved `.tasks` entries are defined without relying on Filer-specific domain names; `default` is available only as an ordinary domain name.
- [x] The design defines a strict, versioned `.tasks/config.json` schema and actionable errors for unreadable files, unsupported versions, unknown fields, duplicate values, and invalid names.
- [x] Each configured domain declares its allowed ID prefixes, while a documented compatibility default applies when project configuration is absent.
- [x] Configured task types declare their checklist behavior and any milestone role, including project-wide milestone-value binding; no behavior is inferred from the type name.
- [x] Tag configuration supports an open policy and a strict catalog policy; open mode preserves today's free-form tags and strict mode rejects unknown tags.
- [x] Configuration precedence and defaults are explicit for the project, each domain, and every CLI command that accepts a domain, prefix, type, or tag; no command obtains an implicit domain from configuration.
- [x] The design places nearest-ancestor discovery in the CLI layer over explicit-root library operations and forbids working-directory or global state inside the library.
- [x] The design specifies typed, serializable errors, warnings, and results for every library operation, including distinct policy-rejection codes, so non-CLI consumers never parse CLI text.
- [x] The design identifies the authoritative CLI, configuration, migration, output-schema, and public Rust documentation that each implementation task must update with its behavior.
- [x] A compatibility and migration plan keeps existing Filer domains and unqualified relationships usable while adopting namespaced identities.
- [x] Examples cover duplicate local IDs, cross-domain relationships, qualified CLI references, custom prefixes, a custom exit-criteria type, and strict tags.
