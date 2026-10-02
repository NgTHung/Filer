# Rust/Filer
---
Filer are file explorer fast alternative with some sprinkle of dev enviroment improvement. 
This is a current WIP project. Proposed sweeping changes that improve long-term maintainability is encouraged.
The project focus is filer-core. Approved companion tracks are the Linux terminal client under tui:TUI-001 and the minimal desktop validation under app:UI-011. Follow the active scope and execution order in docs/task-tracking.md before choosing work. Framework evaluation, the full app rewrite, task-web features, and ecosystem expansion remain deferred.
Product crates live under `crates/` and repository tooling lives under `tools/`. Read the root Cargo.toml before assuming crate paths.

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
You can modify main branch.
Remember to commit every step of a plan.
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
Do not use comment to dividing sections of code(`-------\nABC\n------`). 
Write test in the tests folder instead of inline it.

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

**Markdown in doc comments:**
Rustdoc renders markdown, so doc comments (`///` and `//!`) may use it where it aids the rendered output: code spans, fenced code blocks, lists, and links. Keep it minimal and in service of clarity, not decoration. Plain inline comments (`//`) render as nothing, so they carry no markdown.

**Never use:**
- Placeholder comments ("for now", "TODO: extract this later")
- Markdown formatting (`**bold**`, `_italic_`) in plain `//` inline comments
- ASCII diagrams (put those in `/docs/` if needed)
- Section divider comments (`// ========== Section ==========`)
- Comments explaining removed code during refactors

## Writing Style

Read `docs/WRITING_GUIDE.md` before you write or edit any Markdown document. It owns the full rules and the review checklist. The rules agents break most often:

- Open with the problem the thing solves, not with project status or milestone numbers.
- Describe the system as it is, in the present tense. Keep "now", "still", "no longer", "removed by", and task IDs out of READMEs and reference docs.
- Put status in `.tasks/` and the roadmap, history in commits and dated reviews. Link to them instead of restating them.
- Give each fact one home and link to it.
- Explain why before what, and back performance claims with numbers.
- Use short, active sentences. No em dashes, no filler adjectives, no bold in prose.

## Task Tracking

Filer tracks features, epics, milestones, and development work in `.tasks/`, version-controlled with the code. The `taskroot` CLI validates and queries them.

Use the repository-local `taskroot-workflow` skill for substantial planning or implementation, named task IDs, ready-work selection, and any work that may create or refine a task.

When choosing work, read the Agent Workflow section of `docs/task-tracking.md` for the active scope and scoped ready queue.

Run `taskroot` before and after task changes:

```bash
cargo run -p taskroot -- validate
cargo run -p taskroot -- list
```

Project milestones live in `.tasks/milestones/`. Normal tasks reference one with `milestone: "0.3.0"`.

`docs/task-tracking.md` is the authoritative reference for when to create tasks, the task lifecycle, frontmatter, and every command. Do not restate those rules here.

## Agent skills

### Issue tracker

For issue and spec publication, ticket breakdowns, triage, and wayfinding, use `.tasks/` through `taskroot`. See `docs/agents/issue-tracker.md`.

### Triage labels

Triage uses the configured exclusive category and state tags with the default role names. See `docs/agents/triage-labels.md`.

### Domain docs

Before naming domain concepts or proposing architecture, follow the single-context domain documentation rules. See `docs/agents/domain.md`.
