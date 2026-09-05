# Architecture Invariants

This file owns the rule IDs used by task frontmatter. Reference these IDs in `rules` when a task can affect a project-level constraint.

[Core composition and work lifetime](../adr/0001-core-runtime-lifecycle.md)
records the accepted runtime direction and its open implementation tasks.

## CORE-LIBRARY

`filer-core` is a library. It must not depend on GUI frameworks, HTTP servers, desktop shell UI, or app-specific state.

## PROVIDER-ACCESS

Core filesystem workflows should go through provider contracts. Direct local filesystem shortcuts are only acceptable when the current provider API cannot express the operation.

## SESSION-BOUNDARY

Commands and events that belong to user activity must carry session identity or stay explicitly scoped to one session.

See `state-ownership.md` for how `CORE-LIBRARY` and `SESSION-BOUNDARY` divide state across `filer-app`, `filer-core`, and `filer-ecosystem`.

## ACTOR-LONG-WORK

Navigation, search, preview, operations, and watcher flows should keep long-running work behind actors or actor-like modules with cancellation and structured events.

## PIPELINE-TRANSFORMS

Directory filtering, sorting, and grouping should flow through `Pipeline` and produce `GroupedNodes`.

## WIRE-SAFE-EXTENSIONS

Public extension contracts should use serializable envelopes that can cross process and transport boundaries.

## SEMANTIC-EXTENSION-OUTPUT

Extensions should emit semantic file-manager data, not pixel-level UI instructions.

## CORE-MECHANICS-BUILTIN

Normal navigation, scanning, search dispatch, watching, file operations, provider resolution, sessions, cancellation, cache invalidation, and pipeline execution stay in `filer-core`.
