# Task Tracking

The `.tasks/` directory stores Filer development work as markdown files with YAML frontmatter. `filer-task` validates metadata, links, criteria sections, prefixes, and rule references so task state stays useful during large roadmap migrations.

Use these commands before and after task changes:

```bash
cargo run -p filer-task -- validate
cargo run -p filer-task -- list
cargo run -p filer-task -- list --format json
```

## Task Files

Tasks live under `.tasks/core`, `.tasks/app`, or `.tasks/ecosystem`.

File names must start with the task ID:

```text
.tasks/core/CORE-001-location-routing.md
```

Every task uses frontmatter followed by markdown body sections:

```yaml
---
id: CORE-001
title: Location routing
status: To Do
priority: High
type: Feature
parent: CORE-000
depends_on: [CORE-000]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: Touches public command and event routing.
tags: [core, location]
last_updated: 2026-06-05
---
```

```markdown
## Summary

Explain why this work exists and what outcome it should produce.

## Acceptance Criteria

- [ ] Location routing accepts reconstructable references.
- [ ] Unsupported provider routes return structured errors.
- [ ] Tests cover direct local and unsupported routes.
```

## Frontmatter

Required fields:

| Field | Values |
| --- | --- |
| `id` | `PREFIX-NUMBER`, for example `CORE-001` |
| `title` | At least 5 characters |
| `status` | `To Do`, `In Progress`, `Blocked`, `Done`, `Deferred`, `Obsolete` |
| `priority` | `High`, `Medium`, `Low` |
| `type` | `Milestone`, `Epic`, `Feature`, `Bug`, `Refactor`, `TechDebt`, `TestDebt`, `Design`, `Docs` |

Optional fields:

| Field | Purpose |
| --- | --- |
| `parent` | Parent task ID for hierarchy |
| `depends_on` | Task IDs that must exist and must not form cycles |
| `rules` | Architecture rule IDs from `docs/architecture/invariants.md` |
| `risk` | `High`, `Medium`, or `Low` |
| `impact` | Short description of what the work can affect |
| `tags` | Query labels |
| `whitepaper` | Design reference |
| `last_updated` | `YYYY-MM-DD` |

## Criteria Sections

Criteria stay in the markdown body because they are human work instructions, not query metadata.

`Milestone` and `Epic` tasks must include:

```markdown
## Exit Criteria
```

All other active task types must include:

```markdown
## Acceptance Criteria
```

`Deferred` and `Obsolete` tasks may omit criteria, but they must include:

```markdown
## Rationale
```

## Prefixes

Prefixes are fixed by domain so invalid IDs are caught early.

Core prefixes:

`CORE`, `ACTORS`, `API`, `MODULES`, `PIPELINE`, `SERVICES`, `UTILS`, `VFS`, `REL`, `NAV`, `SEARCH`, `OPS`, `PREVIEW`, `PROVIDER`, `PROTOCOL`

App prefixes:

`UI`, `EXPL`, `SETS`, `SRCH`, `MEDIA`, `NAV`, `PERF`, `A11Y`

Ecosystem prefixes:

`PLUG`, `EXT`, `THEME`, `PROFILE`, `PROVIDER`

## Validation

`cargo run -p filer-task -- validate` checks:

- YAML frontmatter parses into the strict task model.
- IDs and parent IDs use `PREFIX-NUMBER`.
- File names start with the task ID.
- Prefixes are allowed for the task domain.
- Parent tasks exist.
- Dependencies exist, do not duplicate IDs, do not reference self, and do not form cycles.
- Rule IDs exist in `docs/architecture/invariants.md`.
- `last_updated` is a real `YYYY-MM-DD` date.
- `impact` has useful content when present.
- Required criteria or rationale sections exist.

## Workflow

Create tasks when work introduces a feature, capability, significant refactor, architectural bug fix, or whitepaper implementation. Do not create tasks for routine formatting, trivial fixes, existing-doc edits, or dependency bumps.

When starting work, move the task to `In Progress`. When complete, verify the implementation, tests, and criteria before marking it `Done`. Use `Blocked` only when progress depends on a missing decision, external state, or unresolved dependency. Use `Deferred` or `Obsolete` with a clear rationale so future readers know why the work is not active.

List focused task sets with filters:

```bash
cargo run -p filer-task -- list --status "In Progress"
cargo run -p filer-task -- list --priority High
cargo run -p filer-task -- list --domain core
cargo run -p filer-task -- list --parent CORE-000
cargo run -p filer-task -- list --tag location
```

Use JSON output when another tool needs structured data:

```bash
cargo run -p filer-task -- list --format json
```
