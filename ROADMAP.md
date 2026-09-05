# Filer Core Roadmap

This roadmap tracks the core engine and shared project architecture. The desktop
app roadmap lives in `filer-app/ROADMAP.md`; the extension and sync contract
roadmap lives in `filer-ecosystem/ROADMAP.md`.

Current milestone: `0.3.1`, Local excellence. Core contract stabilization in
`0.3.0` is complete. Use the scoped queue in
[task tracking](docs/task-tracking.md#agent-workflow) when choosing work.

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

The current priority is local performance and reliability through the public
core contracts. Finish the remaining allocation and maintenance work, then
record comparable flat-directory baselines and one browse journey. The semantic
extension plane follows in 0.4.0. The small app:UI-011 validation client supplies
real-window feedback alongside core work. Full framework evaluation, task-web
features, and broader benchmark adapters remain later work.

The proof target for this phase is practical: Filer should load a very large
local directory, such as `C:\Windows\System32`, without blocking the client, and
then apply git-style decorations asynchronously in a large repository without
blocking directory loading.

`0.2.3` hardens the additive provider-aware `Location` layer introduced in
`0.2.2`. `LocationRef` now uses explicit id-only, descriptor-only, and full
variants so empty references cannot be constructed, and `LocationDescriptor`
separates the provider root from ordered `LocationSegment` layers.
`LocationId` hashes canonical identity fields, including ordered segments, but
not display-only text. `LocationRoute` now classifies descriptors as direct
local paths, segmented locations, or unsupported provider routes, with registry
caching for the derived route. Public commands, events, `FileNode`, and
`FsProvider` still use their existing path/node surfaces until a later
migration. The intent is for `Location` to become the bridge across local files,
remote providers, virtual providers, extension-backed providers, and archives.
The remaining core stabilization work is tracked in `.tasks/`.

`0.2.4` made the Location/NodeId migration contract explicit and started the
Location-first read/navigation core. The goal was not to remove `NodeId`.
Instead, `Location` became the preferred transport identity for new read-side
work, while `NodeId` remains a compatibility and cache handle for existing
local-path flows.

`0.3.0` is the public-contract cleanup boundary. The first cleanup pass removes
the misleading generic public cancel command in favor of explicit
`CancelSearch`, `CancelScan`, `CancelPreview`, and operation-id scoped
`CancelOperation`. It also renames cancellation errors to `Cancelled` and adds a
stable `TimedOut` code. API-006 removes the path- and NodeId-addressed command
and event compatibility variants, so Location-native results are now the only
public result surface. API-007 and API-017 have retired the remaining FileNode
rows and NodeId-keyed internals. Provider-context timeout propagation is complete.

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

## Active Work

`.tasks/` is the source of truth for work status, dependencies, priorities, and
completion criteria. Do not add active checklists to this file.

Use `taskroot` to inspect the current work:

```bash
cargo run -p taskroot -- validate
cargo run -p taskroot -- list --milestone 0.3.1
cargo run -p taskroot -- milestone 0.3.1 --exit-checklist
cargo run -p taskroot -- list --domain core
cargo run -p taskroot -- list --blocked
cargo run -p taskroot -- summary
```

Task structure and lifecycle commands are documented in
`docs/task-tracking.md`.

## Validation

Run these checks before calling a core milestone complete:

```bash
cargo fmt --check
cargo check -p filer-core
cargo test -p filer-core
cargo check --workspace
```

Run focused tests for current risk areas:

```bash
cargo test -p filer-core watcher -- --nocapture
cargo test -p filer-core scanner_cache_tests -- --nocapture
cargo test -p filer-core preview -- --nocapture
cargo test -p filer-core operator -- --nocapture
```
