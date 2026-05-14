# Filer Core Roadmap

This roadmap tracks the core engine and shared project architecture. The desktop
app roadmap lives in `filer-app/ROADMAP.md`; the extension and sync contract
roadmap lives in `filer-ecosystem/ROADMAP.md`.

## Product Direction

Filer should be a fast Explorer replacement with useful programmer support. It
should remain a file manager first: dependable navigation, file operations,
search, previews, provider support, and extension points that help real work
without turning the project into a full IDE.

The core exists to make that possible across frontends. The Iced app, future web
client, server transport, extension host, and package tools should all consume
the same core behavior instead of reimplementing filesystem logic.

The extension system should improve core mechanics by adding semantic
capabilities around the file-manager kernel. Extensions may contribute commands,
providers, previews, metadata, file decorations, status badges, panels, and
other structured outputs, but core remains the authority for navigation,
scanning, search, operations, sessions, provider access, cache correctness, and
event routing. Clients render extension output; extensions should not depend on
desktop-only UI code.

The current priority is **Core Contract Stabilization**. "Core" means the stable
contracts that prevent churn across the app, future web clients, and extensions:
request identity, cancellation, stale-event rejection, operation identity,
structured error categories, provider addressing, large-directory loading
contracts, extension output envelopes, and local-profile boundaries. It does
not mean building every planned extension surface, web transport, marketplace,
or app rewrite immediately.

The proof target for this phase is practical: Filer should load a very large
local directory, such as `C:\Windows\System32`, without blocking the client, and
then apply git-style decorations asynchronously in a large repository without
blocking directory loading.

Milestone labels:

- `Core`: engine behavior required by every frontend.
- `Reliability`: correctness, cache invalidation, watcher behavior, errors, and
  tests.
- `Power`: programmer-oriented features and advanced file-manager workflows.
- `Protocol`: serialization, server transport, and future web/mobile clients.
- `Ecosystem`: bridge points into `filer-ecosystem`.
- `Future`: valuable work that should not block the main product.

## Architecture Invariants

These constraints are still the project contract.

### Core Is A Library

`filer-core` is not an application. It must not import GUI frameworks, HTTP
servers, desktop shell UI, or app-specific state. Frontends depend on core, not
the other way around.

### All Filesystem Access Goes Through Providers

Core workflows should use `FsProvider` and provider capabilities for list, read,
write, watch, search, and future remote access. Local filesystem shortcuts are
allowed only where the current API cannot yet express the required operation,
and those shortcuts should be tracked as roadmap debt.

### Sessions Are Isolation Boundaries

Every command and event that belongs to user activity must carry a `SessionId`.
Tabs, split panes, web clients, and extension calls should be separate sessions
or explicitly scoped to one.

### Actors Own Long-Running Work

Navigation, search, preview, operations, and watch flows should stay behind
actors or actor-like modules with cancellation and structured events. The UI
should not block on core work.

### The Pipeline Owns Directory Transformations

Directory filtering, sorting, and grouping should flow through `Pipeline` and
produce `GroupedNodes`. Actors should not apply ad hoc sort/group logic.

### Extension Contracts Stay Wire-Safe

`Command::Extension` remains useful for trusted in-process modules, but the
public extension path should move toward serializable envelopes shared with
`filer-ecosystem`.

### Extensions Produce Semantic UI Data

Extensions should not send pixel-level UI instructions such as exact colors,
layout, or widget trees. They should emit client-neutral semantic data: file row
decorations, badges, status kinds, labels, tooltips, action availability,
preview payloads, metadata, and theme token references. Desktop and web clients
translate those semantics into their own visuals.

### Core Mechanics Are Not Optional Plugins

Navigation, scanning, search dispatch, watching, file operations, provider
resolution, sessions, cancellation, cache invalidation, and pipeline execution
stay in `filer-core`. They may be enhanced by extensions, but the app should not
need an extension host to perform normal local file management.

### Modules Are Extension-Aware, Not All External Plugins

Built-in modules remain the reliable kernel. Extension-capable areas grow around
that kernel: git state, metadata providers, preview providers, converters,
external terminal/editor commands, remote provider implementations, file
decorations, themes, icons, panels, and status badges. A module may expose
extension hooks without becoming a third-party plugin itself.

The built-in versus extension-capable split is based on product correctness, not
only resource cost. A feature stays built-in when normal local file management
depends on it. A feature becomes extension-capable when it is optional,
domain-specific, user-customizable, or a provider implementation behind a
core-owned contract. Built-in prototypes are acceptable when they prove a future
extension contract.

### Programmer Features Are Helpful Reading Tools

Programmer-oriented work should help users understand and act on files. Git
status, lightweight metadata, previews, external editor/terminal launchers, and
converter actions fit this direction. Continuous compilation, debugging,
language-server diagnostics, and IDE-style project intelligence are outside the
near-term product boundary.

## Current Baseline

- [x] `Core` `FilerCore` public entry point with module loading, command send,
  and event subscription.
- [x] `Core` Session model with `SessionId`, `SessionManager`, and session
  policies.
- [x] `Core` Actor infrastructure with cancellation shared by scanner,
  searcher, previewer, and operations.
- [x] `Core` `FsProvider` abstraction with local filesystem implementation and
  provider capability flags.
- [x] `Core` File operations for create, copy, move, rename, delete, and folder
  creation.
- [x] `Core` Navigation actor with history and `DirectoryLoaded` events.
- [x] `Core` Pipeline support for hidden filtering, extension filtering, sort,
  and group output.
- [x] `Core` Search query parsing and streamed recursive search.
- [x] `Core` Watcher actor with provider-backed watch/unwatch and debounced
  `FsChanged` events.
- [x] `Core` MIME detection and metadata extraction services.
- [x] `Core` Preview registry, preview cache, preview actor, and text, code,
  image, media, and archive preview providers.
- [x] `Core` Directory cache service with explicit refresh bypass support.
- [x] `Core` Request ids and stale-event guards for scan, search, preview, and
  refresh flows.
- [x] `Core` Operation ids for copy, move, delete, rename, create file, and
  create folder progress, completion, and operation-scoped errors.
- [x] `Core` Structured `ErrorKind` categories on app-facing `Event::Error`
  events, with recoverability derived from the kind.
- [x] `Ecosystem` Trusted in-process command seam through `Command::Extension`.
- [x] `Ecosystem` Shared extension manifest, package, registry, and profile
  operation contracts live in `filer-ecosystem`.
- [ ] `Ecosystem` Define extension output events for semantic file decorations,
  status badges, action state, metadata updates, and client-neutral visual
  hints.

## Core Contract Stabilization

This is the next core phase. It should happen before a major app rewrite, full
extension runtime, web transport, marketplace, or broad provider expansion.

- [x] `Core` Add request ids for scan, search, preview, and refresh flows so
  stale events can be rejected consistently.
- [x] `Core` Add operation ids for copy, move, delete, rename, create file, and
  create folder progress/completion events.
- [x] `Core` Add structured error kinds for current app-facing error events.
- [ ] `Core` Add richer structured error context for collision, unsupported
  provider capability, stale request, and location-targeted cases.
- [ ] `Core` Define a structured provider `Location` model for local, archive,
  remote, virtual, and extension-backed locations. It should include scheme,
  provider/profile id, internal path, optional archive/member path, display
  path, and capability context instead of relying only on string parsing.
- [ ] `Core` Define the large-directory loading contract: paging,
  virtualization hints, incremental loading, or another bounded strategy.
- [ ] `Core` Decide whether `DirectoryLoaded` remains a full listing event or
  evolves into snapshot/page/delta events for very large folders.
- [ ] `Core` Define cancellation and timeout behavior for provider calls,
  previews, search, operations, and future extension calls.
- [ ] `Ecosystem` Define the extension output envelope and first file
  decoration payload before implementing a broad runtime.
- [ ] `Ecosystem` Define scoped core context subscriptions for visible nodes,
  current directory, selection, provider changes, and filesystem changes.
- [ ] `Core` Document which state is app-local config, core session snapshot,
  provider profile reference, extension profile state, and future sync data.
- [ ] `App` Treat current UI bugs as input to core contracts when they reveal
  stale events, duplicate loads, preview races, or large-directory limits.

Exit criteria:

- [x] Stale scan/search/preview results are ignored by request identity.
- [x] Operations emit correlated progress/completion by operation id.
- [x] Error events carry a structured `ErrorKind` so app and web clients can
  branch without parsing message strings.
- [ ] Error events carry enough target/context for app and web clients to render
  clear location-aware feedback.
- [ ] The provider `Location` model is documented and tested.
- [ ] Large-directory loading is bounded and testable.
- [ ] Archive traversal is modeled as provider navigation, not only preview.
- [ ] Undo and conflict-resolution data contracts are drafted, even if full UI
  and behavior come later.
- [ ] The first trusted git decoration prototype emits semantic decorations
  without blocking directory loading.
- [ ] App-local config remains separate from ecosystem profile operations.

## Reliability First

These items should stay ahead of new feature work because the app depends on
core events being truthful and fresh.

- [ ] `Reliability` Add focused regression tests for watcher-driven refresh so a
  created, renamed, or deleted file appears after the app refreshes.
- [ ] `Reliability` Add directory cache invalidation tests for write operations,
  watcher events, manual refresh, and same-folder navigation.
- [ ] `Reliability` Ensure every operation that mutates files invalidates the
  affected parent directories.
- [x] `Reliability` Add stale-event guards for preview and search results by
  session and request identity.
- [ ] `Reliability` Add error targets and provider-specific context so app UI
  can display clear permission, collision, not-found, and unsupported-provider
  states.
- [ ] `Reliability` Keep structured tracing for all app-facing command and event
  paths.
- [ ] `Reliability` Add stress tests for rapid create/delete/rename watcher
  bursts.
- [ ] `Reliability` Add cancellation tests for long operations, search, preview,
  and provider calls.

## Navigation And Sessions

- [x] `Core` Navigate to absolute paths.
- [x] `Core` Back and up navigation.
- [x] `Core` Forward navigation support in app state.
- [x] `Core` Per-session navigation state and event routing.
- [ ] `Core` Add first-class forward navigation command if the app still relies
  on UI-local history.
- [ ] `Core` Add session snapshots that can restore current path, history,
  pipeline config, selection hints, and active providers.
- [ ] `Power` Support tabs as independent sessions.
- [ ] `Power` Support split panes as independent sessions.
- [ ] `Power` Add workspace restore primitives for tabs, panes, paths, provider
  profiles, and layout state.
- [ ] `Future` Add session handoff for web/server transport.

## Directory Pipeline

- [x] `Core` Sort by name, size, modified date, and extension/type.
- [x] `Core` Group by extension/type, date, size, and first letter.
- [x] `Core` Filter hidden files and extensions.
- [x] `Core` Preserve pipeline config across app refreshes.
- [ ] `Core` Add view-independent folder preference model for sort, group,
  hidden files, and density.
- [ ] `Core` Add natural sort and locale-aware comparison options.
- [ ] `Core` Add stable grouping labels for empty extension, folder, and unknown
  type cases.
- [ ] `Power` Add project-aware grouping for source, config, generated, media,
  archive, and document categories.
- [ ] `Core` Add large-directory paging, incremental directory loading, or
  another bounded result contract once the `DirectoryLoaded` direction is
  decided.

## Search

- [x] `Core` Parse text, glob, extension, size, type, hidden, depth, max,
  date, name, and regex search filters.
- [x] `Core` Stream recursive search results with completion state.
- [x] `Core` Cancel previous search work per session.
- [x] `Core` Add search result request ids so stale result batches can be
  discarded safely by all frontends.
- [ ] `Core` Add scoped search roots for selected folders, current folder, and
  workspace/project search.
- [ ] `Power` Add indexed search service for large projects.
- [ ] `Power` Add provider-specific search delegation when a provider advertises
  native search capability.
- [ ] `Ecosystem` Expose search provider contribution points through the
  extension host, while keeping core search cancellation, request identity, and
  result routing authoritative.

## File Operations

- [x] `Core` Create file and folder.
- [x] `Core` Copy, move, rename, delete, and trash-aware delete.
- [x] `Core` Emit operation progress and completion events.
- [x] `Core` Refresh current app state after completed operations.
- [ ] `Reliability` Add conflict-resolution primitives for copy and move.
- [ ] `Reliability` Draft undo and conflict-resolution metadata contracts now so
  operation implementation does not paint future UI into a corner.
- [x] `Reliability` Add operation ids and request correlation for progress,
  cancellation, and app status bars.
- [ ] `Reliability` Add atomic best-effort behavior documentation per provider.
- [ ] `Power` Add operation queue, pause/resume where practical, and operation
  history.
- [ ] `Power` Add undo metadata for reversible operations.
- [ ] `Power` Add bulk rename planning.
- [ ] `Power` Add archive create, extract, compress, and decompress operations.

## Preview And Metadata

- [x] `Core` Preview registry and cache.
- [x] `Core` Text preview.
- [x] `Core` Code preview with syntax highlighting behind feature flag.
- [x] `Core` Image thumbnail preview behind feature flag.
- [x] `Core` Media summary preview.
- [x] `Core` Archive entry preview.
- [x] `Core` Metadata extractors for image, audio, video, document, archive, and
  code categories.
- [x] `Reliability` Add stale preview request ids and selection guards.
- [ ] `Core` Move remaining local-path preview assumptions toward provider or
  provider-backed cache access.
- [ ] `Core` Decide which `FileNode` fields are synchronous guarantees and
  which metadata fields are lazy so grouping/sorting clients do not depend on
  unstable data.
- [ ] `Core` Add richer text/code preview payloads suitable for app rendering
  without locking the app to one highlighter.
- [ ] `Power` Add video preview payloads that can support a player frontend.
- [ ] `Power` Add audio preview payloads that can support a player frontend.
- [ ] `Power` Add markdown/document preview payloads.
- [ ] `Power` Add hex/binary preview payloads.
- [ ] `Power` Add thumbnail disk cache keyed by stable file identity or content
  hash.
- [ ] `Ecosystem` Allow manifest-declared preview and metadata providers to
  register through a core host bridge.
- [ ] `Ecosystem` Allow extensions to publish metadata and preview status as
  structured core events that clients can render consistently.

## Providers And Virtual Filesystems

- [x] `Core` Local filesystem provider.
- [x] `Core` Provider capability model for read, write, watch, and search.
- [ ] `Core` Make provider profile/config types serializable for app and sync.
- [ ] `Core` Introduce provider `Location` as the canonical file identity across
  local files, archives, remote providers, virtual providers, and extension
  providers.
- [ ] `Power` Implement archives as navigable folders.
- [ ] `Power` Implement SFTP/SSH provider.
- [ ] `Power` Implement WebDAV provider.
- [ ] `Power` Implement S3 provider.
- [ ] `Power` Implement vault/encrypted provider.
- [ ] `Power` Add provider connection manager primitives.
- [ ] `Power` Add provider-aware path addressing for local, archive, remote, and
  virtual locations.
- [ ] `Ecosystem` Treat non-local providers as extension-friendly
  implementations of core-owned provider contracts rather than app-specific
  integrations.
- [ ] `Future` Add FUSE mount support for any provider.
- [ ] `Future` Add Kubernetes provider if it still fits the file-manager scope.

## Programmer-Oriented Core Features

- [ ] `Ecosystem` Add git file decoration as the first extension vertical slice:
  visible nodes in, modified/added/deleted/untracked/ignored/conflicted/clean
  decorations out.
- [ ] `Power` Add external terminal and editor command abstractions after the
  command/action contracts are stable.
- [ ] `Future` Add git command primitives for status, diff, branch, commit,
  stash, pull, and push only if they remain helper actions rather than an IDE
  workflow.
- [ ] `Future` Add integrated terminal host contract only if it does not pull
  core or app toward IDE-like project execution.
- [ ] `Power` Add lightweight project detection for repository roots when it
  helps file browsing or git decoration invalidation.
- [ ] `Power` Add converter command contracts for common developer assets and
  documents.
- [ ] `Future` Add task/script launcher primitives only as external command
  launchers, not as a build system or diagnostics runner.
- [ ] `Ecosystem` Prefer extension implementations for git, converters,
  terminal helpers, syntax metadata, and provider extras when the capability is
  not essential core file-manager behavior.
- [ ] `Ecosystem` Add file decoration primitives for git-style states such as
  modified, added, deleted, untracked, ignored, conflicted, and clean.

## Wire Protocol And Web

- [ ] `Protocol` Add serde support for public command, event, node, session,
  metadata, preview, operation, and pipeline types.
- [ ] `Protocol` Add a versioned envelope for command and event transport.
- [x] `Protocol` Add request/response correlation ids where events respond to a
  specific command.
- [ ] `Protocol` Add forward-compatible unknown-field behavior tests.
- [ ] `Protocol` Add `filer-server` as a thin transport crate that depends on
  core.
- [ ] `Protocol` Add WebSocket session lifecycle: connect, create session,
  stream events, destroy session.
- [ ] `Future` Add WASM or TypeScript client bindings over the same protocol.
- [ ] `Future` Add web UI parity with the desktop app once transport is stable.

## Ecosystem Bridge

- [x] `Ecosystem` Keep trusted in-process command routing through
  `Command::Extension`.
- [x] `Ecosystem` Keep module system as the internal composition point for core
  actors and built-ins.
- [ ] `Ecosystem` Prove the extension model with one complete git decoration
  slice before adding broad surfaces such as panels, marketplace, package
  install UI, or full WASM runtime behavior.
- [ ] `Ecosystem` Implement the first host as a trusted in-process prototype or
  trusted core add-on. Do not present it as an untrusted third-party plugin
  system until sandboxing and permission enforcement exist.
- [ ] `Ecosystem` Depend on `filer-ecosystem` from core only when the bridge API
  is ready, not for manifest storage alone.
- [ ] `Ecosystem` Add wire-safe extension command/event envelopes.
- [ ] `Ecosystem` Add a core extension host module that consumes validated
  manifests.
- [ ] `Ecosystem` Add an extension output data plane for decorations, badges,
  panels, context actions, metadata, preview results, and invalidation events.
- [ ] `Ecosystem` Limit early client-facing output to row decorations, status
  badges, context/command actions, preview payloads, and metadata payloads.
  Arbitrary tabs, popups, panels, and layout control should wait until the
  decoration slice proves the data plane.
- [ ] `Ecosystem` Map extension permissions to session policies and provider
  capabilities.
- [ ] `Ecosystem` Add scoped filesystem host calls for extensions.
- [ ] `Ecosystem` Allow extensions to contribute commands, preview providers,
  metadata providers, converters, and providers.
- [ ] `Ecosystem` Allow extensions to subscribe to relevant core context such as
  current directory, visible nodes, selected nodes, provider changes, and file
  change events.
- [ ] `Ecosystem` Route extension logs through core tracing.
- [ ] `Ecosystem` Emit recoverable extension failure events for app UI.
- [ ] `Future` Add WASM runtime host after the wire-safe contract is stable.
- [ ] `Future` Add native trusted bridge for built-ins and explicitly trusted
  power integrations.

## Profile, Sync, And Backup

- [ ] `App` Keep near-term local config simple and app-owned: bookmarks, recent
  paths, theme, layout, panel visibility, density, and sort/group preferences.
- [ ] `Core` Define session snapshots and provider profile references without
  taking ownership of app-only UI persistence.
- [ ] `Ecosystem` Reserve profile operations for extension installs,
  enablement, extension settings, themes/icon packs, provider profiles,
  workspaces, and future sync.
- [ ] `Core` Add provider-profile identifiers that can be referenced by app,
  core, and profile sync without storing secrets in portable data.
- [ ] `Power` Add file sync planning between two providers.
- [ ] `Power` Add conflict strategies for file sync.
- [ ] `Power` Add incremental backup planning.
- [ ] `Future` Add server transport for profile operation sync.

## Themes And Accessibility Support

Most visual theme work belongs in `filer-app`, but core should expose the data
that frontends and extensions need.

- [ ] `Core` Ensure `FileNode`, metadata, and preview payloads carry enough
  information for accessible labels without extra filesystem calls.
- [ ] `Ecosystem` Support serializable theme and icon-pack manifests through
  `filer-ecosystem`.
- [ ] `Core` Avoid embedding app-specific color, icon, or layout assumptions in
  core events.
- [ ] `Protocol` Ensure accessibility-relevant file metadata survives the future
  wire boundary.

## Technical Debt Register

| Issue | Impact | Priority |
| --- | --- | --- |
| Watcher refresh and directory cache behavior need stronger regression coverage | App can receive `FsChanged` but still render stale directory contents | High |
| Preview registry still has local-path assumptions for magic-byte fallback and provider generation | Remote providers and archive providers cannot preview through pure `FsProvider` yet | High |
| Public command/event types are not fully wire-safe or versioned | Blocks web transport and public extension host | High |
| Extension seam still uses `Arc<dyn Any>` for payloads | Fine for trusted in-process modules, unsuitable for ecosystem packages | High |
| Extension contracts describe declarations better than live semantic output | Blocks git-style file badges, status coloring, panel data, and web/app parity | High |
| Provider config/profile model is not settled | Blocks persistent remote providers, sync, and extension-managed providers | Medium |
| Large-directory virtualization/paging is not represented in core | App performance will suffer on very large folders | Medium |
| Directory loading may still assume full snapshots | Very large folders need pages, deltas, or bounded snapshots | Medium |
| Conflict resolution and undo metadata are not modeled | Limits professional file operation workflows | Medium |

## Validation Checklist

Run these before calling a core milestone complete:

```bash
cargo fmt
cargo check -p filer-core
cargo test -p filer-core
cargo check --workspace
```

Targeted checks for current risk areas:

```bash
cargo test -p filer-core watcher -- --nocapture
cargo test -p filer-core scanner_cache_tests -- --nocapture
cargo test -p filer-core preview -- --nocapture
cargo test -p filer-core operator -- --nocapture
```

Manual checks:

- [ ] Create, rename, and delete files externally while the app watches the
  current folder; confirm refreshed listings are fresh.
- [ ] Navigate, refresh, search, sort, group, and preview without losing session
  isolation.
- [ ] Run file operations and confirm cache invalidation updates the current
  directory.
- [ ] Confirm app/core logs show command, event, watcher, preview, search, and
  operation flow clearly.
- [ ] Confirm new core APIs support the app, future web transport, and
  ecosystem contracts without UI-specific assumptions.
