# filer-core

Core library for the Filer file explorer.

Current milestone: `0.2.3`.

## Modules

| Module | Purpose |
|--------|---------|
| `api/` | Public interface (commands, events, handle) |
| `model/` | Data structures (FileNode, FileTree, Query) |
| `actors/` | Concurrent workers (scanner, searcher, watcher, previewer) |
| `bus/` | Message routing between actors |
| `vfs/` | Virtual filesystem abstraction |
| `pipeline/` | Data transformations (filter, sort, group) |
| `services/` | Feature modules (mime, metadata, preview) |
| `utils/` | Shared helpers (path, size, time) |

## Extension Boundary

`filer-core` owns the file-manager kernel: sessions, navigation, scanning,
search dispatch, watching, file operations, provider access, pipeline
execution, cache invalidation, cancellation, and event routing.

Extensions should enhance that kernel rather than replace it. They can add
commands, providers, previews, metadata, file decorations, status badges, and
other semantic outputs through core-controlled contracts. Clients such as the
desktop app or future web UI should render those outputs in their own native
interface.

For example, a git extension should publish semantic file states like
`modified`, `added`, or `untracked`. Core routes those states to clients; the
client decides whether to show an `M` badge, a color token, a tooltip, or a side
panel entry.

## Current Core Priority

The next core milestone is contract stabilization. Before a major app rewrite or
full extension runtime, core should settle:

- provider addressing beyond raw local paths
- richer error targets and provider-specific error cases
- cancellation and timeout semantics
- large-directory loading behavior
- extension output envelopes and file decoration payloads
- the boundary between app-local config, core session snapshots, provider
  profiles, extension profile state, and future sync

Completed contract work:

- request ids and stale-event guards for scan, search, preview, and refresh
- focused stale-event regression tests for scan, search, and preview, including
  Location command parity and parallel test validation
- cache and refresh hardening for direct-path `Location` scans and navigator
  invalidation-triggered refreshes
- operation ids for copy, move, delete, rename, create file, and create folder
  progress, completion, and operation-scoped errors
- structured errors through `ErrorKind`, stable `ErrorCode`, optional
  `ErrorTarget`, `CoreError`, and correlation-aware `Event::Error` helpers
- additive provider-aware `Location` primitives through `LocationId`,
  `LocationDescriptor`, `LocationRef`, `LocationRoute`, `LocationSegment`, and
  `ProviderRef`, with registry recovery from descriptors when id lookup fails
- hardened `LocationRef` transport modes and `LocationId` hashing that ignores
  display-only text

`0.2.3` builds on the `0.2.2` additive `Location` layer by tightening its
identity and transport rules. `LocationRef` now uses explicit enum variants for
id-only, descriptor-only, and full references, so an empty reference cannot be
constructed. `LocationId` hashes identity fields only and ignores
`display_path`. `LocationDescriptor` now separates the provider root from
ordered segment layers so nested archive/member and virtual layers are modeled
without compressing everything into one path string. `LocationRoute` now
classifies descriptors as direct local paths, segmented locations, or
unsupported provider routes, and `NodeRegistry` caches those derived routes by
`LocationId`. Error events now carry formal `ErrorCode` and `ErrorTarget`
fields, and core emits structured `tracing` diagnostics when converting
`CoreError` into `Event::Error`. Direct local `Location` commands are now routed
through navigation, scan, search, preview, metadata, and extended metadata, with
tests covering cancellation, stale-result suppression, cache reuse, and
correlation behavior. This still does not remove the existing public command,
`FileNode`, or `FsProvider` path surfaces. Large-directory loading, timeout
semantics, full `Location` migration, archive navigation, and extension output
envelopes remain open contract work.

Built-in modules should become extension-aware where useful, but navigation,
scan, search orchestration, watch, file operations, sessions, provider routing,
cache, pipeline, and event delivery remain core kernel behavior.

The provider-addressing contract is now structured, not just a custom string
parser. The first `Location` layer represents a scheme, provider reference,
provider-internal root path, and ordered segment stack. `LocationRef` is the
transport-friendly shape: it can carry only a `LocationId` for compact
in-process messages, only a `LocationDescriptor` for reconstruction, or both
for cross-process recovery. The intent is for `Location` to become the bridge
across local files, remote providers, virtual providers, extension-backed
providers, and archives. `LocationRoute` is the first internal routing seam:
direct local paths can be handed to current path-based modules, segmented
locations are recognized but not executed, and profile/ephemeral providers are
reported as unsupported until provider connection routing exists. Archive
navigation, capability context, and full command/event migration are still
future work.

Core stabilization is complete only when large directory loading is bounded,
errors carry enough structured target/context for app and web clients, archive
traversal is modeled as provider navigation, and the trusted git-decoration
prototype proves extension output can arrive after directory data without
blocking it.

## Request IDs

Async command flows that can produce stale results now carry a `RequestId`.
Callers create one request id per user intent and include it on commands for
navigation-driven scans, refresh, search, preview, metadata, and extended
metadata. The same id is echoed on matching events, including
`DirectoryLoaded`, `ScanProgress`, `SearchResults`, `PreviewReady`,
`PreviewFailed`, `MetadataLoaded`, and `ExtendedMetadataLoaded`.

`RequestId::new()` creates runtime-local monotonic ids. `FilerCore` also exposes
`next_request_id()` for callers that prefer to allocate ids through the handle.
`RequestId::DEFAULT` is reserved for compatibility placeholders and should not
be used for new client-originated work.

Scan, search, and preview actors remember the latest request per session. If an
older task finishes after a newer one, its result events are dropped before they
reach clients. Request-scoped errors use `Event::Error { request: Some(id), .. }`;
operation-scoped errors carry both `request: Some(id)` and
`operation: Some(id)` when the originating command has both identifiers.

The test coverage checks that superseded scan, search, and preview requests do
not emit stale client-visible result events, including Location-based scan,
search, and preview commands and parallel test execution.

## Location

`Location` is the additive addressing foundation for provider-aware core work.
It does not replace the current public `PathBuf` and `NodeId` command and event
surfaces yet.

The core types are:

- `LocationId`: stable id derived from a descriptor's identity fields.
- `ProviderRef`: provider identity without credentials, such as `Local` or a
  named profile. `Ephemeral` providers are session-local and must not be used
  as persisted identity unless paired with a reconstructable descriptor.
- `LocationDescriptor`: reconstructable address data: scheme, provider
  reference, provider-internal root path, ordered segments, and optional display
  path.
- `LocationSegment`: an ordered layer after the provider root, such as
  `ArchiveMember { path }` or a future virtual layer.
- `Location`: internal complete form containing both id and descriptor.
- `LocationRef`: wire/transport-friendly enum with `Id`, `Descriptor`, and
  `Full { id, descriptor }` variants.
- `LocationRoute`: internal routing classification for direct local paths,
  segmented locations, and unsupported provider routes.

`LocationId` hashes the scheme, provider reference, provider-internal root path,
and ordered segments. It does not hash `display_path`, so presentation-only
labels do not change identity. `LocationDescriptor::local()` does not call
filesystem canonicalization; provider-specific canonicalization can be added
later as an explicit operation.

Resolution follows a hybrid rule. `LocationRef::Id` uses the registry fast path
or returns a `CoreError` with `ErrorCode::LocationUnresolved`.
`LocationRef::Descriptor` reconstructs and registers the location.
`LocationRef::Full` uses the id when present in the registry and falls back to
descriptor recovery when the id is missing.

Routing is derived from descriptors. Unsegmented `file` + `Local` descriptors
route to `LocationRoute::DirectPath`; segmented local descriptors route to
`LocationRoute::Segmented`; profile and ephemeral providers route to
`LocationRoute::UnsupportedProvider` until provider profile routing exists.
`LocationRoute::require_direct_path()` returns
`LocationSegmentedUnsupported` for segmented routes and `UnsupportedProvider`
for provider routes that are not yet connected. The registry caches derived
routes by `LocationId` and clears that cache with the rest of the registry
state.

Commands or persisted state that may cross a process or machine boundary should
carry descriptors, not ids alone. Large batches can still avoid repeating full
descriptors by sending a parent descriptor once and compact child references
later. Current providers still execute against `PathBuf`; `Location` is the
compatibility layer that lets core migrate toward provider-aware addressing in
small steps.

Direct local `Location` commands currently route through the existing path-based
execution layer. `NavigateLocation`, `ScanLocation`, `SearchLocation`,
`LoadPreviewLocation`, `LoadMetadataLocation`, and
`LoadExtendedMetadataLocation` resolve their `LocationRef`, require a direct
path route, and preserve the original request id on results or structured
errors. Segmented and unsupported-provider routes are represented and reported,
but not executed yet.

Scanner, searcher, and previewer tests now cover Location parity for stale
result suppression, cancellation, cache hits, and session isolation. A
`ScanLocation` cache hit emits `DirectoryEntriesLoaded`; `RefreshNode` still
bypasses cache after a Location scan. Navigator invalidation now records
navigated nodes and triggers `RefreshNode` for sessions currently displaying the
invalidated directory, so app- or watcher-driven invalidation refreshes fresh
directory data instead of serving stale cache entries.

Nested archive addresses are represented as a provider root plus ordered
segments, for example:

```text
sftp profile "work" -> /home/me/bundle.zip -> archive member vendor.tar -> archive member src/main.rs
```

That shape preserves each VFS layer: the outer provider, each archive boundary,
and the final member path. The `root` field is the provider-internal entry path
for the first layer; archive/member traversal remains a later migration target.
Segmented local descriptors do not expose a direct `as_local_path()` because the
full address no longer maps to one filesystem path.

## Operation IDs

File operation commands now carry an `OperationId`. Callers create one operation
id per user intent and include it on `Copy`, `Move`, `Delete`, `Rename`,
`CreateFolder`, and `CreateFile` commands. The same id is echoed on
`OperationProgress` and `OperationComplete` events.

`OperationId::new()` creates runtime-local monotonic ids. `FilerCore` also
exposes `next_operation_id()` for callers that prefer to allocate ids through
the handle. `OperationId::DEFAULT` is reserved for compatibility placeholders
and should not be used for new client-originated work.

Operation-scoped failures use `Event::Error { operation: Some(id), request:
Some(request), .. }` through `Event::from_operation_error()`. Non-operation
errors use `operation: None`. Cancellation remains session-scoped for now;
operation-id-specific cancellation can be added later without changing the
correlation contract.

The operation tests assert that progress, completion, and error paths preserve
the originating operation id. Collision and provider-failure cases for write
operations use the same request/operation correlation contract as successful
operations.

## Errors

App-facing error events carry formal error fields:

```rust
Event::Error {
    kind,
    code,
    target,
    message,
    recoverable,
    session,
    request,
    operation,
}
```

`message` remains the human-readable display text. `kind` is the broad category
for coarse UI branching. `code` is the stable machine-readable reason clients
should prefer for specific behavior. `target` identifies the failed object when
core can name one, such as a path, location id, provider, session, request,
operation, actor, or channel. `recoverable` is derived from `ErrorCode`, so
event producers do not maintain separate boolean mappings.

Current kinds are:

- `Io`
- `NotFound`
- `PermissionDenied`
- `InvalidPath`
- `InvalidLocation`
- `ChannelClosed`
- `Cancelled`
- `Actor`
- `Network`
- `InvalidData`
- `InvalidInput`
- `Unsupported`
- `Unknown`

Current codes include path, location, provider, session, navigation, channel,
actor, network, data, input, unsupported-operation, and unknown cases.
`CoreError` is structured data rather than a variant-only enum; construct it
through helpers such as `CoreError::not_found(path)`,
`CoreError::permission_denied(path)`, `CoreError::location_unresolved(id)`, and
`CoreError::from_io_error(err, path)`.

Use `Event::from_error()`, `Event::from_request_error()`, and
`Event::from_operation_error()` instead of constructing `Event::Error` by hand.
Those helpers preserve request/operation correlation and emit one structured
`tracing` event with `error.kind`, `error.code`, `error.target`,
`error.recoverable`, and `error.message`. `filer-core` does not install a
global tracing subscriber; applications and tests decide how to collect logs.

## Usage

```rust
use std::path::PathBuf;

use filer_core::{Command, Event, FilerCore, RequestId};

#[tokio::main]
async fn main() {
    let core = FilerCore::new().await.unwrap();

    core.send(Command::Handshake).unwrap();
    let session = match core.event_receiver().recv().unwrap() {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    };

    let request = RequestId::new();

    // Send command
    core.send(Command::Navigate {
        path: PathBuf::from("/home"),
        session,
        request,
    })
    .unwrap();

    // Receive events
    while let Ok(event) = core.event_receiver().recv() {
        match event {
            Event::DirectoryLoaded {
                groups,
                request: loaded_request,
                ..
            } if loaded_request == request => {
                println!("Loaded {} files", groups.total_count);
            }
            _ => {}
        }
    }
}
```

## Dependencies

- `tokio` — Async runtime
- `flume` — Channels for actor communication
- `async-trait` — Async trait support
