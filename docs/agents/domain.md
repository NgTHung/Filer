# Domain Docs

Engineering skills read Filer's domain documentation before naming concepts or proposing architectural changes.

## Before exploring

Read these resources when they exist:

- `CONTEXT.md` at the repository root
- Relevant ADRs under `docs/adr/`

Proceed silently when either resource is absent. Create or update them through `domain-modeling` when terminology or architectural decisions are resolved.

## Layout

Filer uses one domain context across its root-level Rust workspace.

- `CONTEXT.md` defines shared domain terms.
- `docs/adr/` records repository-wide architectural decisions.

The workspace crates remain direct children of the repository root. Domain documentation does not change crate ownership or the current filer-core focus.

## Vocabulary

Use terms defined in `CONTEXT.md` in tasks, specifications, tests, and code.

When a concept is missing, first check whether Filer already uses another term. Record a real vocabulary gap through `domain-modeling`.

## ADR conflicts

Surface any proposal that conflicts with an existing ADR. Name the ADR and explain why its decision may need reconsideration instead of silently overriding it.
