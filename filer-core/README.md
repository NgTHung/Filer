# filer-core

Core library for the Filer file explorer.

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

- structured recoverable errors
- provider addressing beyond raw local paths
- cancellation and timeout semantics
- large-directory loading behavior
- extension output envelopes and file decoration payloads
- the boundary between app-local config, core session snapshots, provider
  profiles, extension profile state, and future sync

Completed contract work:

- request ids and stale-event guards for scan, search, preview, and refresh
- operation ids for copy, move, delete, rename, create file, and create folder
  progress, completion, and operation-scoped errors

Built-in modules should become extension-aware where useful, but navigation,
scan, search orchestration, watch, file operations, sessions, provider routing,
cache, pipeline, and event delivery remain core kernel behavior.

The provider-addressing contract should be structured, not just a custom string
parser. A future `Location` should be able to represent a scheme, provider or
profile id, internal path, optional archive/member path, display path, and
capabilities. This keeps local, archive, remote, virtual, and extension-backed
files addressable through the same core model.

Core stabilization is complete only when large directory loading is bounded,
recoverable errors are structured, archive traversal is modeled as provider
navigation, and the trusted git-decoration prototype proves extension output can
arrive after directory data without blocking it.

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
