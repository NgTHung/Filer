# filer-core

Core library for the Filer file explorer.

Current milestone: `0.2.2`.

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
  parallel test validation
- operation ids for copy, move, delete, rename, create file, and create folder
  progress, completion, and operation-scoped errors
- structured error kinds through `ErrorKind`, `CoreError::kind()`, and
  `Event::Error { kind, message, recoverable, session, request, operation }`
- additive provider-aware `Location` primitives through `LocationId`,
  `LocationDescriptor`, `LocationRef`, and `ProviderRef`, with registry
  recovery from descriptors when id lookup fails

`0.2.2` builds on the `0.2.0` correlation and error-category contracts and the
`0.2.1` stale-event reliability patch. It adds `Location` as a compatibility
layer for provider-aware addressing without changing the existing public
command, event, `FileNode`, or `FsProvider` path surfaces. It is not the end of
core stabilization; large-directory loading, richer error context,
cancellation/timeout semantics, full `Location` migration, and extension output
envelopes remain open contract work.

Built-in modules should become extension-aware where useful, but navigation,
scan, search orchestration, watch, file operations, sessions, provider routing,
cache, pipeline, and event delivery remain core kernel behavior.

The provider-addressing contract is now structured, not just a custom string
parser. The first `Location` layer represents a scheme, provider reference, and
provider-internal path. `LocationRef` is the transport-friendly shape: it can
carry only a `LocationId` for compact in-process messages, only a
`LocationDescriptor` for reconstruction, or both for cross-process recovery.
The intent is for `Location` to become the bridge across local files, remote
providers, virtual providers, extension-backed providers, and archives.
Archive/member paths, nested archive stacks, capability context, and full
command/event migration are still future work.

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
non-request errors and operation errors currently use `request: None`.

The `0.2.1` test coverage now checks that superseded scan, search, and preview
requests do not emit stale client-visible result events, including under
parallel test execution.

## Location

`Location` is the new additive addressing foundation for provider-aware core
work. It does not replace the current public `PathBuf` and `NodeId` command and
event surfaces yet.

The core types are:

- `LocationId`: stable id derived from a `LocationDescriptor`.
- `ProviderRef`: provider identity without credentials, such as `Local` or a
  named profile.
- `LocationDescriptor`: reconstructable address data: scheme, provider
  reference, provider-internal path, and optional display path.
- `Location`: internal complete form containing both id and descriptor.
- `LocationRef`: wire/transport-friendly form with optional id and optional
  descriptor.

Resolution follows a hybrid rule. If a `LocationRef` carries an id and the
registry has it, core uses that fast path. If lookup fails but the descriptor is
present, core reconstructs and registers the location. If both lookup and
descriptor recovery fail, core returns `CoreError::InvalidLocation` with
`ErrorKind::InvalidLocation`.

Commands or persisted state that may cross a process or machine boundary should
carry descriptors, not ids alone. Large batches can still avoid repeating full
descriptors by sending a parent descriptor once and compact child references
later. Current providers still execute against `PathBuf`; `Location` is the
compatibility layer that lets core migrate toward provider-aware addressing in
small steps.

Nested archive support should extend `LocationDescriptor` with structured
segments instead of encoding the whole chain into one path string. A future
descriptor should be able to represent a provider root plus ordered segments,
for example:

```text
sftp profile "work" -> /home/me/bundle.zip -> archive member vendor.tar -> archive member src/main.rs
```

That shape preserves each VFS layer: the outer provider, each archive boundary,
and the final member path. The current `path` field is the provider-internal
root path for the first layer; archive/member traversal remains a later
migration target.

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
None, .. }`. Non-operation errors use `operation: None`. Cancellation remains
session-scoped for now; operation-id-specific cancellation can be added later
without changing the correlation contract.

## Error Kinds

App-facing error events now include a machine-readable `ErrorKind`:

```rust
Event::Error {
    kind,
    message,
    recoverable,
    session,
    request,
    operation,
}
```

`message` remains the human-readable display text. `kind` is the stable value
clients should branch on. `recoverable` is derived from `kind` by
`ErrorKind::is_recoverable()` so event producers do not maintain a separate
mapping.

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
- `Unknown`

`CoreError::kind()` maps every `CoreError` variant to one of those categories,
and `Event::from_error()` copies that kind into `Event::Error`. Request-scoped
errors still use `request: Some(id)`, operation-scoped errors still use
`operation: Some(id)`, and generic session errors keep both fields as `None`.
Future work can add structured error targets once the provider `Location` model
is wired through public commands and events.

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
