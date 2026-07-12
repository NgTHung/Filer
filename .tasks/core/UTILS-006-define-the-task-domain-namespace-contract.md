---
id: UTILS-006
title: Define project configuration and namespace contracts
status: To Do
priority: High
type: Design
parent: UTILS-005
risk: High
impact: "Defines task identity, configurable validation policy, and compatibility rules consumed by every command."
tags: [tooling, tasks, namespaces, configuration]
last_updated: 2026-07-12
---

## Summary

Specify the portable project contract before changing stored references and validation. The design must cover namespace identity and a versioned `.tasks/config.json` for domains, domain prefixes, task types, tag policy, and existing Filer repositories. It must also define the library API boundary so a non-CLI consumer, such as a multi-project web UI, can drive every operation.

## Acceptance Criteria

- [ ] The design defines a canonical qualified task identity syntax for CLI arguments, frontmatter references, human output, and JSON output.
- [ ] The design requires an explicit domain in every CLI task reference, restricts unqualified frontmatter references to the containing domain, and defines every rejection and missing-reference error.
- [ ] The design defines how task creation accepts a qualified ID as shorthand for separate domain and local-ID arguments, requires an explicit domain, and rejects conflicting domain inputs.
- [ ] Task identity is specified as domain plus local ID, with local IDs unique only within their domain.
- [ ] Valid domain names, milestone handling, and reserved `.tasks` entries are defined without relying on Filer-specific domain names; `default` is available only as an ordinary domain name.
- [ ] The design defines a strict, versioned `.tasks/config.json` schema and actionable errors for unreadable files, unsupported versions, unknown fields, duplicate values, and invalid names.
- [ ] Each configured domain declares its allowed ID prefixes, while a documented compatibility default applies when project configuration is absent.
- [ ] Configured task types declare their checklist behavior and any milestone role needed by validation, lifecycle, readiness, and milestone commands.
- [ ] Tag configuration supports an open policy and a strict catalog policy; open mode preserves today's free-form tags and strict mode rejects unknown tags.
- [ ] Configuration precedence and defaults are explicit for the project, each domain, and every CLI command that accepts a domain, prefix, type, or tag; no command obtains an implicit domain from configuration.
- [ ] The design places nearest-ancestor discovery in the CLI layer over explicit-root library operations and forbids working-directory or global state inside the library.
- [ ] The design specifies typed, serializable errors and results for every library operation so non-CLI consumers present them without parsing CLI text.
- [ ] The design identifies the authoritative CLI, configuration, migration, output-schema, and public Rust documentation that each implementation task must update with its behavior.
- [ ] A compatibility and migration plan keeps existing Filer domains and unqualified relationships usable while adopting namespaced identities.
- [ ] Examples cover duplicate local IDs, cross-domain relationships, qualified CLI references, custom prefixes, a custom exit-criteria type, and strict tags.
