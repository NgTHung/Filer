The `.tasks/` directory contains a structured task tracking system that manages the entire development lifecycle of Filer. Tasks are defined as markdown files with YAML frontmatter and validated against a JSON schema.

## Overview

Task tracking provides:
- **Hierarchical organization** with epics and subtasks
- **Status tracking** across the development lifecycle
- **Metadata** like priority, assignee, and whitepaper references
- **Validation** via a CLI tool to ensure consistency

### Directory Structure

```
.tasks/
├── core/                           # Core - Filer heart
│   ├── CORE-000-vfs-core.md
│   ├── PIPELINE-001-update-filer.md
│   ├── VFS-000-local-fs.md
│   ├── INDEX-*.md
│   ├── LSYNC-*.md
│   └── ...
├── app/                            # App - iced frontend
│   ├── UI-000-interface-v2.md
│   ├── EXPL-000-explorer-epic.md
│   ├── SETS-000-settings-epic.md
│   └── ...
├── ecosystem/                      # Ecosystem - Extension, etc
│   └── (future tasks)
├── task.schema.json                # JSON schema defining task structure
└── AGENTS.md                       # Task tracking cheatsheet
```

Tasks follow the naming pattern: `{PREFIX}-{NUMBER}-{slug}.md` and are organized into subdirectories by domain (core, app, ecosystem).

## Task Schema

Every task file contains YAML frontmatter that must conform to the schema defined in `task.schema.json`.

### Required Fields

```yaml
---
id: CORE-001
title: VFS model
status: Done
priority: High
---
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier matching pattern `^[A-Z]+-[0-9]+$` |
| `title` | string | Human-readable title (min 5 characters) |
| `status` | enum | One of: `To Do`, `In Progress`, `Done` |
| `priority` | enum | One of: `High`, `Medium`, `Low` |

### Optional Fields

```yaml
---
parent: CORE-000
tags: [core, database, epic]
whitepaper: Section 4.1
last_updated: 2025-12-02
---
```

| Field | Type | Description |
|-------|------|-------------|
| `parent` | string | Parent task ID for subtask hierarchy |
| `tags` | array | Categorization tags |
| `whitepaper` | string | Reference to design documentation |
| `last_updated` | string | ISO date of last modification |

## Task Statuses

The lifecycle of a task follows three states:

### To Do
Task has not been started. No implementation exists.

### In Progress
Task is actively being worked on. Criteria for this status:
- Some code has been written
- Feature is partially implemented
- Still has rough edges or incomplete functionality

### Done
Task is complete. All acceptance criteria must be met:
- All listed acceptance criteria are implemented
- Code is merged to main branch
- Tests pass (if applicable)
- Feature is production-ready

#### Note
Never mark a task as `Done` if implementation is partial, tests are failing, or the feature doesn't work as specified.

## Task Prefixes

Tasks are organized by domain using prefixes. Tasks are further organized into subdirectories:

### Core Tasks (`.tasks/core/`)

Backend and Rust-related tasks:

| Prefix | Domain |
|--------|--------|
| `CORE` | Core architecture and data model |
| `ACTORS` | Actors model |
| `API` | API model(e.g. commands/events) |
| `MODULES` | Basic modules(e.g. navigation/watch) |
| `PIPELINE` | Basic data pipeline model |
| `SERVICES` | Extra services(e.g. metadata/mime) |
| `UTILS` | Common shared functionality |
| `VFS` | Core File system mechanics |

### App Tasks (`.tasks/app/`)

Frontend and Iced-related tasks:

| Prefix | Domain |
|--------|--------|
| `UI` | UI primitives and design system |
| `EXPL` | Explorer interface |
| `SETS` | Settings pages |
| `SRCH` | Search UI |
| `MEDIA` | Media viewer |
| `NAV` | Navigation and routing |
| `PERF` | Performance optimizations |
| `A11Y` | Accessibility |

### Ecosystem Tasks (`.tasks/ecosystem/`)

Ecosystem-specific tasks (future):

| Prefix | Domain |
|--------|--------|
| `PLUG` | Plugin system |

## Task Validator CLI

The `task-validator` binary provides utilities for managing and validating tasks.

### List Tasks

View all tasks sorted by status:

```bash
cargo run --bin task-validator -- list --sort-by status
```

Output groups tasks by status (To Do, In Progress, Done):

```
=== Done ===
CORE-000: Epic: VDFS Core Architecture
CORE-001: Entry-Centric Data Model
JOB-001: Job Manager for Task Scheduling

=== In Progress ===
CLOUD-003: Cloud Volume Support

=== To Do ===
AI-002: Create Fine-tuning Dataset
```

### Filter Tasks

Filter by specific criteria:

```bash
# By status
cargo run --bin task-validator -- list --status "In Progress"

# By priority
cargo run --bin task-validator -- list --priority High

# By assignee
cargo run --bin task-validator -- list --assignee james
```

### Validate Schema

Ensure all task files conform to the schema:

```bash
cargo run --bin task-validator -- validate
```

This checks:
- YAML frontmatter is valid
- All required fields are present
- Field values match allowed enums
- ID pattern is correct
- Parent references exist

Validation runs in CI to prevent invalid tasks from being committed.

## Workflow

### Reviewing Task Status

When code has been merged, tasks should be reviewed and updated:

1. **List current task state**:
   ```bash
   cargo run --bin task-validator -- list --sort-by status
   ```

2. **For each potential completed feature**:
   - Read the task file to understand acceptance criteria
   - Read all implementation files mentioned in the task
   - Check for integration tests
   - Verify each acceptance criterion is met in the code

3. **Update task status**:
   Edit the YAML frontmatter in the task file:
   ```yaml
   ---
   status: Done  # Changed from "In Progress"
   last_updated: 2025-12-02  # Update date
   ---
   ```

4. **Validate changes**:
   ```bash
   cargo run --bin task-validator -- validate
   ```

#### Tip
When unsure if a task is complete, leave it as `In Progress` rather than prematurely marking it `Done`. The tracker is only useful if it's accurate.

### Creating New Tasks

1. Choose appropriate subdirectory (`core/`, `app/`, or `ecosystem/`)
2. Choose appropriate prefix based on domain
3. Find next available number (e.g., if `EXPL-002` exists, use `EXPL-003`)
4. Create file with pattern: `.tasks/{subdirectory}/{PREFIX}-{NUMBER}-{slug}.md`
5. Add YAML frontmatter with all required fields
6. Write task description and acceptance criteria
7. Validate:
   ```bash
   cargo run --bin task-validator -- validate
   ```

### Updating Task Status

When updating tasks after completing work:

**DO:**
- Read implementation files thoroughly
- Verify all acceptance criteria are met
- Check for passing integration tests
- Update `last_updated` field
- Be rigorous about what "Done" means

**DON'T:**
- Mark tasks done based on assumptions
- Skip reading the actual code
- Ignore failing tests or partial implementations
- Batch update tasks without verification

## Common Patterns

### Epic Tasks

Epics are high-level tasks that have subtasks:

```yaml
---
id: CORE-000
title: "Epic: VFS Core Architecture"
status: Done
priority: High
tags: [epic, core, vdfs]
---
```

Subtasks reference their epic via the `parent` field:

```yaml
---
id: CORE-001
title: Entry-Centric Data Model
parent: CORE-000
status: Done
---
```

### Task Dependencies

While not enforced by schema, dependencies can be documented in task descriptions:

```markdown
## Dependencies

This task requires:
- CORE-001 (Entry-Centric Model) - Done
- INDEX-001 (Location Watcher) - In Progress
```

### Whitepaper References

Link tasks to design documentation:

```yaml
whitepaper: Section 4.1
```

This helps trace implementation back to architectural decisions.

## Best Practices

### Task Granularity

- **Epics**: High-level features spanning multiple subtasks
- **Tasks**: Concrete features implementable in focused work
- **Too granular**: Don't create tasks for every file or function

### Acceptance Criteria

Write specific, testable criteria:

**Good:**
```markdown
- [ ] User can add S3 bucket as a cloud volume
- [ ] Cloud volumes appear in volume list
- [ ] Files in cloud volume can be searched
```

**Bad:**
```markdown
- [ ] Implement cloud volume feature
- [ ] Make it work properly
```

### Status Accuracy

The tracker is only valuable if it reflects reality:

- Review and update tasks regularly
- Don't mark tasks done to "clean up" the list
- Keep `In Progress` honest about active work
- Archive or remove truly obsolete tasks

## Troubleshooting

### Validation Errors

**Invalid YAML**:
```
Error: Failed to parse YAML in CORE-001-entry-centric-model.md
```
Fix: Check YAML syntax, ensure frontmatter is enclosed in `---`

**Missing Required Field**:
```
Error: Missing required field 'priority' in CORE-001
```
Fix: Add the missing field to frontmatter

**Invalid Status Value**:
```
Error: Invalid status 'Complete' in CORE-001. Must be one of: To Do, In Progress, Done
```
Fix: Use exact status values from schema (case-sensitive)

### Duplicate IDs

Each task ID must be unique. If validation reports duplicates:

```bash
# Find all task IDs
grep "^id:" .tasks/*.md | sort
```

Renumber conflicting tasks and update any parent references.

### Orphaned Tasks

Tasks with `parent` fields referencing non-existent tasks:

```bash
cargo run --bin task-validator -- validate
```

Will report broken parent references. Either remove the parent field or create the missing epic.
