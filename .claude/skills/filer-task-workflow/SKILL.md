---
name: filer-task-workflow
description: Use when planning or implementing substantial Filer work, when a request names a task ID, when choosing ready work, or when work may require creating or refining a task.
---

<!-- Mirror of .agents/skills/filer-task-workflow/SKILL.md (Codex). Keep both copies identical; symlinks are not portable on this repo. -->

# Filer Task Workflow

Make `.tasks/` the durable source of intent. Use `filer-task` JSON for decisions.

## Enter the Workflow

At the start of planning or implementation:

1. Run `cargo run -q -p filer-task -- validate`.
2. Inspect the named task with `context TASK-ID --format json`, or search with `ready` and `list --format json`.
3. Read `docs/task-tracking.md` only for command details.
4. Compose with relevant available planning, TDD, debugging, execution, review, and verification skills.

Do not skip task-state inspection for trivial work. Trivial work may require no task mutation, but stale task state still changes planning.

## Decide Task Work

Create or refine a task for:

- a feature or capability
- a significant refactor
- an architectural bug fix
- a whitepaper or specification implementation

Do not create a task for routine formatting, a trivial fix, an existing-document update, or a dependency bump.

Before creating a task, prove none covers the outcome. Before refining one, load its context and related work.

Apply YAGNI:

- describe the required outcome, not a speculative architecture
- write observable criteria that can be verified
- add relationships and metadata only from repository evidence
- keep dependency implementation outside scope unless the user explicitly expands scope

Use `add` for new tasks. Edit task markdown directly only to refine existing intent, then validate immediately.

## Respect Mode and Intent

- In non-mutating planning, put the exact task creation or refinement first in the execution plan.
- Planning, review, and explanation do not start tasks.
- For implementation intent, start a ready `To Do` task before production changes.
- Resume an `In Progress` task without another lifecycle mutation.
- Do not start a task with readiness blockers. Report them and stay within scope.
- Use `block` only for a genuine external decision or unavailable state.
- Never defer or obsolete work without explicit user intent.

## Implement Against Context

Load `context TASK-ID --format json` before implementation. Treat the task summary, criteria, declared relationships, and embedded rules as constraints.

Use relevant installed methodology skills when available. If none are available, follow repository TDD and verification rules directly.

If implementation reveals missing scope:

1. stop expanding code
2. refine the task or propose a separate task
3. validate task state
4. resume only after the intended scope is explicit

## Complete With Evidence

Before completion:

1. Run focused tests, then the required crate checks.
2. Review the diff against every criterion and repository size guidance.
3. Change checklist criteria to checked only when evidence proves them.
4. Run `cargo run -q -p filer-task -- validate`.
5. Run `done TASK-ID` only after all required criteria are checked.
6. Validate again and inspect `show TASK-ID`.

Never use passing task validation as a substitute for code verification.
