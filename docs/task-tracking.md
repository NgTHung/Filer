# Task Tracking

The `.tasks/` directory stores Filer development work as markdown files with YAML frontmatter. `filer-task` validates metadata, links, criteria sections, prefixes, and rule references so task state stays useful during large roadmap migrations.

This guide describes the behavior available in the repository today. The approved [task project contract](task-project-contract.md) defines planned portable domains, qualified identities, and project configuration. Update this guide with each implementation task so examples never lead the behavior they describe.

Use these commands before and after task changes:

```bash
cargo run -p filer-task -- validate
cargo run -p filer-task -- list
cargo run -p filer-task -- summary
cargo run -p filer-task -- list --format json
```

## Project Discovery

`filer-task` starts at your current working directory and selects the nearest
ancestor that directly contains a `.tasks` directory. You can run commands from
the project root or any nested path. A nested project takes precedence over an
outer project.

Use `--root <path>` on any command to start discovery somewhere else. Relative
paths resolve from your current working directory, and the path may point to a
project root, a nested directory, or an existing file inside the project.

```bash
cargo run -p filer-task -- list --root ../another-project/src --format json
cargo run -p filer-task -- validate --root C:\work\another-project
```

The `.tasks` directory alone marks a project. It does not need
`task.schema.json`. Discovery does not inspect task contents, so malformed task
files produce validation errors for the nearest project instead of causing a
search for an outer project.

If no `.tasks` directory exists at or above the starting path, the command exits
unsuccessfully and reports both the searched path and the required `.tasks`
directory.

## Task Files

Tasks live under `.tasks/core`, `.tasks/app`, or `.tasks/ecosystem`. Project milestones live under `.tasks/milestones`.

File names must start with the task ID:

```text
.tasks/core/CORE-001-location-routing.md
.tasks/milestones/MILESTONE-003-core-contract-stabilization.md
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
milestone: "0.3.0"
depends_on: [CORE-000]
rules: [CORE-LIBRARY, PROVIDER-ACCESS]
risk: High
impact: Touches public command and event routing.
tags: [core, location]
last_updated: 2026-06-05
---
```

Milestone tasks are project references, not domain-local IDs. A milestone file uses a normal task ID with the `MILESTONE` prefix and stores the shared milestone value in `milestone`:

```yaml
---
id: MILESTONE-003
title: Core contract stabilization
status: In Progress
priority: High
type: Milestone
milestone: "0.3.0"
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

Status and type are separate. Status records lifecycle state. Type classifies the work and selects its criteria heading. `Deferred` and `Obsolete` are statuses, not task types. They require `## Rationale` and may omit criteria. `Blocked` is also a status; it requires `## Blocked Reason` in addition to the criteria selected by the task type.

The current implementation assigns exit criteria and milestone behavior to built-in type names. The approved project contract removes that name-based assumption. Configured types will declare checklist behavior and the milestone role explicitly.

Optional fields:

| Field | Purpose |
| --- | --- |
| `parent` | Parent task ID for hierarchy |
| `milestone` | Project milestone value, for example `0.3.0` |
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

Tasks with any other type must include the following section unless their status is `Deferred` or `Obsolete`:

```markdown
## Acceptance Criteria
```

Tasks whose status is `Deferred` or `Obsolete` may omit criteria, but they must include:

```markdown
## Rationale
```

`Blocked` tasks must include:

```markdown
## Blocked Reason
```

`Done` tasks must not have unchecked checklist items in `## Acceptance Criteria` or `## Exit Criteria`.

## Prefixes

Prefixes are fixed by domain so invalid IDs are caught early.

Core prefixes:

`CORE`, `ACTORS`, `API`, `MODULES`, `PIPELINE`, `SERVICES`, `UTILS`, `VFS`, `REL`, `NAV`, `SEARCH`, `OPS`, `PREVIEW`, `PROVIDER`, `PROTOCOL`

App prefixes:

`UI`, `EXPL`, `SETS`, `SRCH`, `MEDIA`, `NAV`, `PERF`, `A11Y`

Ecosystem prefixes:

`PLUG`, `EXT`, `THEME`, `PROFILE`, `PROVIDER`

Milestone prefixes:

`MILESTONE`, only under `.tasks/milestones`

## Validation

`cargo run -p filer-task -- validate` checks:

- YAML frontmatter parses into the strict task model.
- IDs and parent IDs use `PREFIX-NUMBER`.
- File names start with the task ID.
- Prefixes are allowed for the task domain.
- Parent tasks exist.
- Referenced milestones match exactly one milestone task.
- The `MILESTONE` prefix appears only under `.tasks/milestones`.
- Dependencies exist, do not duplicate IDs, do not reference self, and do not form cycles.
- Rule IDs exist in `docs/architecture/invariants.md`.
- `last_updated` is a real `YYYY-MM-DD` date.
- `impact` has useful content when present.
- Required criteria, blocked reason, or rationale sections exist.
- `Done` tasks have no unchecked criteria items.

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
cargo run -p filer-task -- list --milestone 0.3.0
cargo run -p filer-task -- list --blocked
```

Use JSON output when another tool needs structured data:

```bash
cargo run -p filer-task -- list --format json
```

## Agent Workflow

Use `ready` to select executable work. A ready task is `To Do`, is not a milestone, has no child tasks, has only `Done` dependencies, and has only `To Do` or `In Progress` ancestors. Results sort by priority and then task ID.

```bash
cargo run -p filer-task -- ready
cargo run -p filer-task -- ready --domain core --milestone 0.3.0 --limit 5
cargo run -p filer-task -- ready --tag provider --format json
```

Use `show` when you need one task's full metadata and body sections:

```bash
cargo run -p filer-task -- show PROVIDER-001
cargo run -p filer-task -- show PROVIDER-001 --format json
```

Use `context` before implementation. It returns the target task, readiness blockers, direct task relationships, milestone, referenced architecture rule text, and whitepaper path. It does not infer source files.

```bash
cargo run -p filer-task -- context PROVIDER-001
cargo run -p filer-task -- context PROVIDER-001 --format json
```

New agent-oriented JSON responses include `schema_version: 1`. Existing command JSON stays unchanged.

An agent should use this sequence:

```bash
cargo run -p filer-task -- ready --limit 5 --format json
cargo run -p filer-task -- context PROVIDER-001 --format json
cargo run -p filer-task -- start PROVIDER-001
# Implement and test the task.
cargo run -p filer-task -- validate
cargo run -p filer-task -- done PROVIDER-001
```

Inspect dependencies that still need work:

```bash
cargo run -p filer-task -- deps --incomplete CORE-042
cargo run -p filer-task -- deps --incomplete CORE-042 --format json
```

Inspect milestone exit criteria and progress:

```bash
cargo run -p filer-task -- milestone 0.3.0 --exit-checklist
cargo run -p filer-task -- milestone 0.3.0 --exit-checklist --format json
```

Generate progress summaries:

```bash
cargo run -p filer-task -- summary
cargo run -p filer-task -- summary --milestone 0.3.0
cargo run -p filer-task -- summary --format json
```

Use lifecycle commands to keep status and rationale sections consistent:

```bash
cargo run -p filer-task -- add --domain core --id CORE-042 --title "Provider timeout propagation" --priority High --type Feature --milestone 0.3.0
cargo run -p filer-task -- add --domain milestones --id MILESTONE-003 --title "Core contract stabilization" --priority High --type Milestone --milestone 0.3.0
cargo run -p filer-task -- start CORE-042
cargo run -p filer-task -- done CORE-042
cargo run -p filer-task -- block CORE-042 "Waiting for provider timeout policy decision."
cargo run -p filer-task -- defer CORE-042 "No longer needed for the current milestone."
cargo run -p filer-task -- obsolete CORE-042 "Replaced by CORE-044."
```

Successful human output uses the same headings, labels, and path format across commands. Paths are relative to the repository and use `/` separators:

```text
Task Started
Task: CORE-042
Path: .tasks/core/CORE-042-provider-timeout-propagation.md
```

Validation and imports use labeled summaries:

```text
Validation
Status: Passed
Tasks: 23
```

```text
Import
Mode: Dry Run
Tasks: 2

Paths
.tasks/milestones/MILESTONE-003-core-contract-stabilization.md
.tasks/core/CORE-042-provider-timeout-propagation.md
```

`add` can scaffold richer task files when a migration already knows the metadata:

```bash
cargo run -p filer-task -- add --domain core --id CORE-042 --title "Provider timeout propagation" --priority High --type Feature --parent MILESTONE-003 --milestone 0.3.0 --rule PROVIDER-ACCESS --risk High --impact "Touches provider calls and cancellation behavior." --tag provider --summary "Propagate provider deadlines through core calls." --criterion "Provider calls receive timeout context."
```

Use `--criterion` for open checklist items and `--checked-criterion` when creating a `Done` task with completed criteria. `Blocked` tasks need `--blocked-reason`. `Deferred` and `Obsolete` tasks need `--rationale`.

## Batch Import

Use `import` when migrating curated roadmap items into `.tasks/` without writing each markdown file by hand. The input is JSON and uses the same field names as task frontmatter, plus `summary`, `criteria`, `rationale`, and `blocked_reason` for body sections:

```json
[
  {
    "domain": "milestones",
    "id": "MILESTONE-003",
    "title": "Core contract stabilization",
    "priority": "High",
    "type": "Milestone",
    "milestone": "0.3.0",
    "criteria": [{ "text": "Public contracts are named consistently." }]
  },
  {
    "domain": "core",
    "id": "CORE-042",
    "title": "Provider timeout propagation",
    "priority": "High",
    "type": "Feature",
    "parent": "MILESTONE-003",
    "milestone": "0.3.0",
    "rules": ["PROVIDER-ACCESS"],
    "risk": "High",
    "impact": "Touches provider calls and cancellation behavior.",
    "tags": ["provider"],
    "summary": "Propagate provider deadlines through core calls.",
    "criteria": [{ "text": "Provider calls receive timeout context." }]
  }
]
```

Validate the batch before writing:

```bash
cargo run -p filer-task -- import docs/roadmap-migration.tasks.json --dry-run
```

Write the batch once dry run passes:

```bash
cargo run -p filer-task -- import docs/roadmap-migration.tasks.json
```

Use `--skip-existing` for reruns after a partial manual migration. Import validates the whole batch before writing files, including parent, dependency, milestone, and rule references.
