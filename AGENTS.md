# Rust/Filer
---
Filer are file explorer fast alternative with some sprinkle of dev enviroment improvement. 
This is a current WIP project. Proposed sweeping changes that improve long-term maintainability is encouraged.
The current repo/project focus is on Filer-core, we want to fully focus on Filer-core crate without bothering others crate(even filer-app and filer-ecosystem).

## Core Priority
---
1. Performance first.
2. Reliability.
If a tradeoff must be made, choose correctness and robustness over short-term convenience.

## Maintainability
---
Long term maintainability is a core priority so if you add new functionality, first check if there are shared logic can be extracted to a seperate module. Duplicated logic across multiple files is a code smell and should be avoided. Don't be afraid to change existing code. Don't take shortcut by just adding local logic to solve a problem.

## Filer-core
---
Filer-core is the heart of this project.
This is a TDD project, you shouldn't write the implement blindly without the test written throughly/correctly.
Follow existing code style. ALWAYS read and copy the style of similar tests when adding new cases.
All changes must be tested.
Avoid large module:
- Prefer adding new modules instead growing existing ones.
- Target Rust modules under 700 LoCs.
- If a file exceeds roughly 1000 LoCs, add new functionality in a new module instead of extending the existing file unless there is a strong documented reason not to.

### Change size guidance
---
Unless the change is mechanical the total number of changed lines should not exceed 1000 lines. For complex logic changes the size should be under 700 lines.
If the change is larger, explore whether it can be split into reviewable stages and identify the smallest coherent stage to land first. Base the staging suggestion on the actual diff, dependencies, and affected call sites.

### Rust rules
Do not use `.unwarp()` / `.expect()` in production code.
Exceptions must be validate/tested fully.
Prefer `Result + ?` or explicit handling.
Do not ignore errors silently.
Avoid unnecessary `.clone()`.
Prefer borrowing when practical.
Do not add dependencies unless needed.
Keep code simple and idomatic.

### Documentation rules
Core principle: Explain WHY, not WHAT. Keep comments as short as possible. One sentence explaining rationale beats a paragraph restating code.

**Module docs(//!)**
- Add a title with # for the module name
- Explain what the module does in plain language (not bullet points)
- Include design rationale naturally in prose
- Add runnable code examples showing usage

**Inline comments:**
- Delete comments that restate obvious code
- Explain WHY for decisions, not WHAT the code does
- Use one sentence when possible
- Only expand for truly non-obvious consequences

**Error handling comments:**
Explain strategy and recovery, not just "log and continue".

**Platform-specific comments:**
Explain consequences, not implementation blockers.

**Never use:**
- Placeholder comments ("for now", "TODO: extract this later")
- Markdown formatting (`**bold**`, `_italic_`) in code comments
- ASCII diagrams (put those in `/docs/` if needed)
- Section divider comments (`// ========== Section ==========`)
- Comments explaining removed code during refactors

## Writing Style

This applies to all documentation, code comments, and design documents.

Use clear, simple language. Write short, impactful sentences. Use active voice. Focus on practical, actionable information.

Address the reader directly with "you" and "your". Support claims with data and examples when possible.

Avoid these constructions:

- Em dashes (use commas or periods)
- "Not only this, but also this"
- Metaphors and cliches
- Generalizations
- Setup language like "in conclusion"
- Unnecessary adjectives and adverbs
- Emojis, hashtags, markdown formatting in prose

Avoid these words:
comprehensive, delve, utilize, harness, realm, tapestry, unlock, revolutionary, groundbreaking, remarkable, pivotal

## Task Tracking

Filer uses a file-based task system in `/.tasks/` to track features, epics, and development work. All task files are version-controlled alongside the code.

### When to Create Tasks

Create tasks for work that:

- Introduces a new feature or capability
- Refactors a significant system or module
- Fixes a bug requiring architectural changes
- Implements a whitepaper specification

Do not create tasks for:

- Routine code formatting or style fixes
- Trivial bug fixes (single line changes)
- Documentation updates to existing features
- Dependency version bumps

### Task Lifecycle

1. Create task file in `/.tasks/` with `status: "To Do"`
2. Update status to `"In Progress"` when you start work
3. Complete implementation and tests
4. Update status to `"Done"` and commit

Full documentation: `/docs/task-tracking.md`
