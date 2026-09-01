# Task project contract

`taskroot` must identify the same task in a shell command, a task file, JSON output, and a long-lived library process. This contract defines that identity and the project policy that validates it. Later implementation tasks must follow this contract instead of adding command-specific rules.

This document specifies the target behavior for UTILS-005. The current behavior remains documented in `task-tracking.md` until each child task implements and documents its part of this contract.

## Task identity

A task identity contains a domain and a local ID. Write its canonical form as `domain:LOCAL-ID`.

```text
core:UTILS-006
default:WORK-001
release:REL-010
```

The colon separates the namespace from the local ID. It keeps the identity in one shell argument and one URL path segment. Human and JSON output must use the same form.

### Domain names

A domain name must:

- contain 1 to 64 ASCII characters
- start with a lowercase letter
- contain only lowercase letters, digits, and single hyphens
- end with a lowercase letter or digit

The names `core`, `release-tools`, and `default` are valid. The names `Core`, `-core`, `core-`, `core--tools`, and `config.json` are invalid.

Windows device names are also invalid, matched without regard to case: `con`, `prn`, `aux`, `nul`, `com1` through `com9`, and `lpt1` through `lpt9`. The loader rejects them with `config_invalid_value` before any directory access. This rule applies on every platform so a project remains portable.

`default` has no special behavior. A command never selects it when you omit a domain.

### Local IDs

A local ID keeps the existing `PREFIX-NUMBER` form. A prefix contains 1 to 32 uppercase ASCII letters or digits and starts with a letter. The number contains one or more ASCII digits. Leading zeroes remain valid.

Local IDs must be unique by exact string inside one domain. Numeric segments are not normalized, so `WORK-001` and `WORK-1` are distinct local IDs. Different domains may contain the same local ID:

```text
core:WORK-001
app:WORK-001
```

These are separate tasks in validation, graph traversal, lifecycle changes, filtering, and output.

### Stored references

The `parent` and `depends_on` fields accept local or qualified references. A local reference resolves only inside the containing task's domain. A qualified reference may target any configured domain.

```yaml
id: WORK-002
parent: WORK-001
depends_on: [release:REL-001]
```

If this task lives in `core`, its parent is `core:WORK-001` and its dependency is `release:REL-001`. A configured project must not search other domains for the unqualified parent.

The `milestone` field remains a milestone value such as `0.3.0`. It is not a task reference. A parent or dependency that targets a milestone task follows the normal identity rules.

## CLI contract

Every CLI argument that selects one task requires a qualified identity. This applies to `show`, `context`, `deps`, `start`, `done`, `block`, `defer`, and `obsolete`.

```bash
taskroot show core:UTILS-006
taskroot done release:REL-010
```

An unqualified lookup fails. If that local ID exists, the error lists every matching qualified identity. If it does not exist, the error still states that the domain is required.

Task creation supports two equivalent forms:

```bash
taskroot add --id core:WORK-001 [OTHER OPTIONS]
taskroot add --domain core --id WORK-001 [OTHER OPTIONS]
```

When `--id` is qualified, it supplies the domain and local ID. A matching `--domain core` is allowed. A conflicting `--domain app` fails with `domain_conflict`. An unqualified `--id` without `--domain` fails with `domain_required`.

The `add --parent` and `add --depends-on` values follow stored-reference rules because the command writes them into frontmatter. An unqualified value resolves in the new task's explicit domain. A qualified value may target another domain. Imports apply the same rule using each imported task's domain.

Commands that do not select one task keep domain arguments optional when they are filters. For example, `list` and `ready` list every domain unless you pass `--domain`.

Malformed identities, invalid domains, unknown domains, missing tasks, ambiguous legacy references, and conflicting creation inputs are separate errors. Each error must include the rejected input and the project root when available.

## Status and type

Status and type describe different parts of a task. Status controls its lifecycle. Type classifies the work and selects configured checklist behavior.

Statuses are fixed library behavior:

| Status | Meaning and required body state |
| --- | --- |
| `To Do` | The work has not started. The configured criteria section is required. |
| `In Progress` | Work is active. The configured criteria section is required. |
| `Blocked` | Work cannot proceed. `## Blocked Reason` and the configured criteria section are required. |
| `Done` | Work is complete. The configured criteria section must have no unchecked items. |
| `Deferred` | Work is intentionally postponed. `## Rationale` is required, and criteria may be omitted. |
| `Obsolete` | Work is no longer relevant. `## Rationale` is required, and criteria may be omitted. |

`Deferred` and `Obsolete` are statuses, not built-in task types. A project can configure a task type with either name, but that type does not change lifecycle status or rationale rules.

Type names carry no behavior. The `criteria` and `role` fields are the only source of type behavior. A type named `Milestone` has no milestone behavior unless its configuration declares `"role": "milestone"`. A type named `Epic` may use acceptance criteria. A type named `ReleaseGate` may carry the milestone role.

## Project layout

The nearest ancestor that directly contains `.tasks` is a task project. Project discovery belongs to the CLI convenience layer. Library operations receive an explicit project root and never perform discovery.

A configured project uses these top-level entries:

```text
.tasks/
  config.json
  DOMAIN/
    LOCAL-ID-title.md
```

`.tasks/config.json` is reserved for project policy. `.tasks/task.schema.json` is a reserved compatibility file. Neither name can be a domain. Names that start with a dot are invalid domains under the domain grammar.

Every configured domain maps to one direct child directory of `.tasks`. A directory that looks like a domain but is not declared in configuration is a validation error. This prevents a misspelled directory from hiding tasks. Markdown files outside configured domain directories are also validation errors.

## Configuration version 1

Configuration lives at `.tasks/config.json`. Version 1 uses this shape:

```json
{
  "version": 1,
  "domains": {
    "default": {
      "prefixes": ["WORK"]
    }
  },
  "task_types": {
    "Feature": {
      "criteria": "acceptance"
    },
    "Epic": {
      "criteria": "exit"
    }
  },
  "tags": {
    "policy": "open"
  }
}
```

All objects are strict. An unknown field is an error at every level. Null values are not accepted. The loader must detect duplicate JSON object keys before a normal map deserializer can discard them.

### Domains and prefixes

`domains` is a non-empty object keyed by domain name. Each domain contains one field, `prefixes`. The prefix list must be non-empty and contain no duplicates.

The same prefix may appear in different domains. A task ID is valid when its prefix belongs to the containing domain. Configuration never supplies an implicit domain.

### Task types

`task_types` is a non-empty object keyed by task type name. A type name contains 1 to 64 ASCII letters or digits, starts with an uppercase letter, and contains no spaces or punctuation.

Every type declares one checklist behavior:

| `criteria` | Required heading |
| --- | --- |
| `acceptance` | `## Acceptance Criteria` |
| `exit` | `## Exit Criteria` |

A type may also set `"role": "milestone"`. A project may define at most one milestone-role type. The role drives milestone validation and milestone commands. It does not reserve a domain, prefix, or directory.

Every task whose type carries the milestone role must include a non-empty `milestone` value. Milestone-role tasks must use unique milestone values across the whole project, regardless of domain. Every task with a `milestone` field must match exactly one milestone-role task with the same value across the project. The milestone-role task matches its own value. Milestone commands resolve that project-wide value, not a domain-local value.

```json
{
  "ReleaseGate": {
    "criteria": "exit",
    "role": "milestone"
  }
}
```

Status rules override checklist behavior. In particular, `Deferred` and `Obsolete` may omit criteria, while `Blocked` and `Done` still apply their status requirements to the configured criteria section.

### Tags

Open policy accepts every syntactically valid tag:

```json
{
  "tags": { "policy": "open" }
}
```

Strict policy accepts only values in `allowed`:

```json
{
  "tags": {
    "policy": "strict",
    "allowed": ["backend", "needs-triage", "ready-for-agent"],
    "exclusive_groups": {
      "triage-state": ["needs-triage", "ready-for-agent"]
    }
  }
}
```

A tag contains 1 to 64 lowercase ASCII letters, digits, or single hyphens. It starts and ends with a letter or digit. The strict catalog may be empty, which forbids all tags. Catalog values must be unique. Open policy must omit `allowed`; strict policy must include it.

`exclusive_groups` is optional under a strict policy. Group names follow tag
name syntax, group values must be unique allowed tags, and a task may select at
most one value from each group.

### Complete custom example

```json
{
  "version": 1,
  "domains": {
    "backend": {
      "prefixes": ["API", "WORK"]
    },
    "release": {
      "prefixes": ["REL", "WORK"]
    }
  },
  "task_types": {
    "Feature": {
      "criteria": "acceptance"
    },
    "Container": {
      "criteria": "exit"
    },
    "ReleaseGate": {
      "criteria": "exit",
      "role": "milestone"
    }
  },
  "tags": {
    "policy": "strict",
    "allowed": ["backend", "release", "security-review"],
    "exclusive_groups": {}
  }
}
```

This project may contain both `backend:WORK-001` and `release:WORK-001`. A `ReleaseGate` task may live in either domain when its local ID uses an allowed prefix.

## Configuration validation

The loader reads configuration once when it opens a project. It validates configuration before reading or writing any task. A failed load leaves the project unchanged.

Version 1 rejects:

- unreadable or invalid JSON with the config path and parser location
- unsupported versions with the received and supported versions
- unknown fields with their JSON path
- duplicate object keys or repeated array values with their JSON path and value
- missing or empty required objects and prefix lists
- invalid domain, prefix, type, or tag names with the rejected value
- more than one milestone-role type with every conflicting type name
- `allowed` under open tag policy or a missing `allowed` under strict policy
- exclusive groups under an open policy or group values absent from `allowed`

Project configuration has one source and no merge layers. When `config.json` exists, it defines the complete domain, prefix, type, and tag policy. CLI values select from that policy and never override it. When the file is absent, the compatibility profile supplies the complete policy.

CLI and imported values follow the same policy:

| Value | Resolution |
| --- | --- |
| Domain | A qualified ID or `--domain` names it explicitly. Unknown names fail. |
| Prefix | The selected domain's prefix list validates the local ID. No flag can add a prefix. |
| Task type | `--type`, imports, and stored tasks must name a configured type. |
| Tag | Add, import, edits, stored-task validation, and tag filters apply the configured policy and exclusive groups. |

Filtering by an unknown domain or strict-policy tag is an input error rather than an empty result. This keeps a misspelled filter from looking like a valid query with no matches.

## Compatibility profile

An absent `config.json` selects the Filer legacy profile. It defines these domains and prefixes:

| Domain | Prefixes |
| --- | --- |
| `core` | `CORE`, `ACTORS`, `API`, `MODULES`, `PIPELINE`, `SERVICES`, `UTILS`, `VFS`, `REL`, `NAV`, `SEARCH`, `OPS`, `PREVIEW`, `PROVIDER`, `PROTOCOL` |
| `app` | `UI`, `EXPL`, `SETS`, `SRCH`, `MEDIA`, `NAV`, `PERF`, `A11Y` |
| `ecosystem` | `PLUG`, `EXT`, `THEME`, `PROFILE`, `PROVIDER` |
| `milestones` | `MILESTONE` |

The profile defines `Milestone` and `Epic` with exit criteria. `Milestone` carries the milestone role. `Feature`, `Bug`, `Refactor`, `TechDebt`, `TestDebt`, `Design`, and `Docs` use acceptance criteria. Tags use open policy. This profile is fixed compatibility data, not a default that configured projects inherit or override.

The profile keeps current task repositories readable, but it does not restore unqualified CLI lookups. CLI task references remain qualified after namespace support lands.

Legacy frontmatter resolution works in this order:

1. Resolve an unqualified reference in the containing domain.
2. If it is missing there, resolve it only when exactly one task in the project has that local ID.
3. If several domains match, return `ambiguous_reference` with every candidate.
4. If none match, return `task_not_found` with the containing domain and local ID.

This fallback exists only when `config.json` is absent. It preserves relationships such as the current `core:CORE-001` parent reference to `milestones:MILESTONE-003`.

Every reference resolved by step 2 emits a `legacy_global_reference` warning. The warning includes the source task, field, local reference, and resolved qualified identity. Validation still succeeds, but human and JSON output expose the warning. Adding the same local ID to another domain turns that warning into `ambiguous_reference`, so projects can migrate fallback-dependent references before they break.

Before Filer adds explicit configuration, migrate every cross-domain relationship to a qualified reference. Same-domain references may remain local. Add the configuration and relationship changes together so no committed state depends on the compatibility fallback after strict mode begins.

## Library boundary

The public library opens one explicit project root and resolves its immutable policy once:

```rust
let project = TaskProject::open(root)?;
let task = project.show(TaskIdentity::parse("core:UTILS-006")?)?;
```

`TaskProject` owns the canonical root and resolved `ProjectPolicy`. Read, validation, graph, import, creation, and lifecycle operations receive `&TaskProject` or are methods on it. They must not call `current_dir`, discover ancestors, or read process-global project state.

`discover_project_root(start)` is a separate helper for CLI and host applications. Opening several roots creates independent project values. Clones of one `TaskProject` share its loaded revision, but each root has independent revision and in-process coordination state.

Every non-dry-run mutation takes an exclusive operating-system lock on `.tasks/.taskroot.lock`. The persistent empty file is a reserved project entry. Separate handles and processes that use `taskroot` therefore serialize writes to the same canonical root without blocking other roots. Process termination releases the operating-system lock; the file itself carries no owner or recovery state.

`TaskProject` hashes `.tasks/config.json` and every Markdown file below `.tasks` when it opens. `is_stale()` compares current content with that baseline, so detection does not depend on file size or timestamp precision. Successful mutations refresh the shared baseline for every clone. A stale mutation fails with `project_stale` before validation or writing. Consumers recover by calling `reload()`, replacing the old handle, rebuilding any derived view, and retrying only if the original intent remains valid under the new policy and task state.

Task creation writes a temporary file in the destination directory, flushes and synchronizes complete content, then persists it without replacing an existing path. Lifecycle updates use the same preparation and atomically replace the destination. Readers can observe the complete old task or complete new task, never a partially written task. A failed preparation or persistence removes the temporary file and preserves the existing destination.

The filesystem lock coordinates cooperating consumers. Editors and tools that ignore it can still race after a freshness check. Their changes are detected by the next content comparison, so consumers must retain normal file-conflict handling for uncoordinated writers.

Public identity and result types expose:

| Field | Meaning |
| --- | --- |
| `domain` | Namespace name |
| `id` | Local `PREFIX-NUMBER` value |
| `qualified_id` | Canonical `domain:LOCAL-ID` value |

Human output uses `qualified_id` whenever it names a task. JSON task objects retain `domain` and local `id`, then add `qualified_id`.

Allowing duplicate local IDs across domains changes the meaning of `id`, even though `qualified_id` is an additive field. UTILS-009 must bump every versioned agent-facing envelope from `schema_version: 1` to `schema_version: 2` when namespace identity lands. Version 2 consumers must key tasks by `qualified_id` or by the `domain` and `id` pair. UTILS-009 must mark unversioned task-array JSON as a semantic breaking change and document the same key migration.

## Structured errors

Library errors serialize a stable `code`, a human `message`, and code-specific context. Consumers branch on `code` and fields, never on `message` or a Rust variant name.

```json
{
  "code": "ambiguous_reference",
  "message": "task reference WORK-001 matches more than one domain",
  "context": {
    "reference": "WORK-001",
    "candidates": ["backend:WORK-001", "release:WORK-001"]
  }
}
```

The public error catalog includes:

| Code | Required context |
| --- | --- |
| `project_not_found` | searched start path |
| `project_stale` | canonical project root |
| `config_io` | config path and operation |
| `config_invalid_json` | config path and parser location |
| `config_unsupported_version` | received and supported versions |
| `config_unknown_field` | JSON path and field |
| `config_duplicate` | JSON path and value |
| `config_invalid_value` | JSON path, value, and constraint |
| `domain_required` | rejected local ID |
| `domain_conflict` | qualified domain and flag domain |
| `invalid_reference` | rejected reference and constraint |
| `unknown_domain` | domain and configured domains |
| `unknown_type` | rejected type and configured types |
| `tag_rejected` | rejected tag, policy, and allowed tags when strict |
| `prefix_not_allowed` | rejected prefix, domain, and allowed prefixes |
| `task_not_found` | qualified identity or local reference context |
| `ambiguous_reference` | reference and qualified candidates |
| `validation_failed` | structured validation issues |
| `io` | path and operation |

Add and import return `unknown_type`, `tag_rejected`, or `prefix_not_allowed` directly when the corresponding input fails policy. A strict-policy tag filter returns `tag_rejected`; an unknown domain filter returns `unknown_domain`. Repository validation returns `validation_failed` and attaches the corresponding code to each stored-task issue. Validation issues keep structured paths, fields, values, and task identities where available. CLI rendering converts the same errors to actionable text. The web UI maps codes to HTTP responses without parsing messages.

Validation warnings use the same stable-code shape without failing the operation:

| Code | Required context |
| --- | --- |
| `legacy_global_reference` | source qualified identity, field, local reference, and resolved qualified identity |

## Documentation ownership

Each implementation task updates behavior and its corresponding documentation in the same change:

| Task | Required documentation |
| --- | --- |
| UTILS-007 | CLI help, root discovery Rust docs, and current task-tracking usage |
| UTILS-008 | Domain loading, qualified creation examples, and `default` semantics |
| UTILS-009 | CLI references, frontmatter relationships, human output, and JSON fields |
| UTILS-010 | Configuration path, version, defaults, validation, and Rust policy API |
| UTILS-011 | Prefix, type, milestone-role, tag policy, and taxonomy migration guidance |
| UTILS-012 | Project isolation, write coordination, atomic writes, and reload recovery |
| UTILS-020 | Exclusive tag-group configuration, validation, mutation, and triage usage |

This document remains the normative contract. `task-tracking.md` remains the current user guide and must change only when the matching behavior lands.
