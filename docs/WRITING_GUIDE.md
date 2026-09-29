# Writing Guide

This guide covers Filer documentation, code comments, and design documents. Each document answers one reader question. You decide which question before you write, and the answer decides what belongs on the page and what goes somewhere else.

## Start with the problem

Open with the problem the thing solves, then describe how it solves it. Do not open with project status, milestone numbers, or what changed recently.

The opening paragraph is two to four sentences. It says what the thing is, who uses it, and which problem it removes for them.

Weak opening:

> Current milestone: `0.3.0`. The active stabilization work is provider-aware core behavior before a larger app rewrite.

Strong opening:

> `filer-core` is the file-manager engine shared by every Filer client. It lists, searches, previews, watches, and changes files through one command and event API, so each client renders results without its own filesystem logic.

## Pick the document type

Each type has one job. Status, history, and plans belong in the types built for them.

| Type | Answers | Contains | Never contains |
|---|---|---|---|
| Root README | What is Filer and how do I run it? | Purpose, crate map, build and run commands, links | Milestones, task IDs, change history, deferred-work lists |
| Crate README | What does this crate do and how do I use it? | Problem, responsibilities, main concepts, one usage example, links to deeper docs | Completed/open/deferred checklists, removal notes |
| Reference guide | How does this behave today? | Present-tense rules, options, commands, limits | Plans, history, "will" statements |
| Contract or design doc | What must an implementation do, and why? | Problem, constraints, decision, rules, rejected alternatives | Task progress |
| ADR | Why did we choose this? | Context, decision, consequences, date | Later status updates |
| Review, baseline, plan | What did we find or intend on a given date? | Dated findings, task IDs, measurements | Edits that track later state |
| Roadmap | Where is the project heading? | Direction, milestone goals | Per-task status, API history |
| Task (`.tasks/`) | What work remains and when is it done? | Scope, criteria, status | Nothing restricted |

Status lives in `.tasks/` and the roadmap. Other documents link there instead of restating it.

## Describe what is, not what changed

Reference docs and READMEs describe the current system in the present tense. Ask of each sentence: will this still be true, and still worth reading, in six months? If not, it belongs in a commit message, a task, or a dated review.

Drop these words from reference docs: now, still, currently, no longer, at this stage, has been, was removed, former, legacy, new.

| Instead of | Write |
|---|---|
| API-006 removed the path-addressed commands, so `Location` is now the only public addressing contract. | Every public command addresses files with `LocationRef`. |
| `LocationRef` now has explicit id-only, descriptor-only, and full modes instead of optional fields. | `LocationRef` has three variants: `Id`, `Descriptor`, and `Full`. |

When a removal breaks callers, write a migration note in the changelog or release notes. Delete migration sections from reference docs one release after the removal.

## Explain why before what

Readers can read the code for what it does. Give them the reason the code cannot give: the constraint, the cost, or the failure it prevents.

Weak: `ListingOptions::metadata()` stats each entry.
Strong: Use `ListingOptions::metadata()` only when you display size or timestamps, because it stats every entry and costs one syscall per row.

Put numbers behind performance and reliability claims. "Retains at most 16,384 rows per chain" is useful. "Bounded memory" is not.

## Keep one home per fact

Each fact has one owning document. Other documents link to it.

- The current milestone lives in `ROADMAP.md`.
- Glossary terms live in `CONTEXT.md`.
- Invariant IDs live in `docs/architecture/invariants.md`.
- Command and event reference lives in the `filer-core` rustdoc and README.

Before you add a paragraph, search for the fact. If it exists, link to it or move it.

## Structure

Use headings that match the reader's task, such as "Load a directory" or "Cancel work". Keep titles short.

Use prose for reasoning and cause and effect. Use lists for parallel, discrete items such as options, commands, or error codes. A list item that needs a "because" clause belongs in prose.

Use a table when the reader compares items across the same attributes.

Use a code example when it replaces a paragraph of explanation. Examples in rustdoc must compile as doctests. Examples in Markdown show the smallest call that works.

Do not use ASCII or Mermaid diagrams. Do not use bold or italic in running prose.

## Length

A root README fits on one screen, around 60 lines. A crate README stays under 250 lines; move detailed reference into rustdoc or a `docs/` page. If a section explains a subsystem in more than five paragraphs, it deserves its own document.

## Sentences

Write short sentences in active voice. Address the reader as "you". State facts plainly and let data carry the weight.

Avoid these constructions:

- Em dashes. Use commas or periods.
- "Not only X, but also Y".
- Metaphors and clichés.
- Generalizations without an example or a number.
- Setup phrases such as "in conclusion" or "it is worth noting".
- Adjectives and adverbs that add no fact, such as fast, simple, clean, robust, powerful, or seamless, unless you measure them.
- Emojis and hashtags.

Avoid these words and their relatives: comprehensive, delve, utilize, harness, realm, tapestry, unlock, revolutionary, groundbreaking, remarkable, pivotal.

## Code comments

Code comments follow the documentation rules in `AGENTS.md`. They explain why, stay short, and never narrate refactor history.

## Review checklist

Before you commit a document, check that:

1. The first paragraph states the problem and fits in four sentences.
2. The document matches one type in the table and contains nothing from its "Never contains" column.
3. No sentence depends on "now", "still", or a task ID to make sense, unless the document is dated.
4. Every performance or reliability claim has a number or a link to a measurement.
5. Every fact that exists elsewhere is a link, not a copy.
