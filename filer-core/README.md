# filer-core

Core library for the Filer file explorer.

Current milestone: `0.3.0`.

## Scope

`filer-core` owns the file-manager kernel:

- sessions and command/event routing
- navigation, scanning, SearchNodeCompat, watching, preview, and metadata dispatch
- file operations
- provider access and directory loading
- pipeline execution for filter, sort, and group
- request/operation correlation, stale-result rejection, cancellation, cache
  invalidation, and app-facing errors

Extensions should enhance the kernel through core-controlled contracts. They can
contribute providers, previews, metadata, file decorations, status badges, and
semantic outputs. Clients decide how to render those outputs.

## Modules

| Module | Purpose |
|---|---|
| `api/` | Public commands, events, and handle |
| `model/` | Shared data types |
| `actors/` | Scanner, searcher, watcher, previewer, and operation workers |
| `bus/` | Message routing |
| `vfs/` | Provider and filesystem abstraction |
| `pipeline/` | Filter, sort, group, and paging policy |
| `services/` | MIME, metadata, preview, and cache services |
| `utils/` | Shared helpers |

## Current Contracts

The active stabilization work is provider-aware core behavior before a larger
app rewrite or full extension runtime.

Completed:

- request ids and stale-result guards for ScanPathCompat, SearchNodeCompat, preview, metadata, and
  refresh flows
- operation ids for CopyNodeCompat, MoveNodeCompat, DeleteNodeCompat, RenameNodeCompat, create file, and create folder
- structured app-facing errors with stable `ErrorCode`, optional `ErrorTarget`,
  request/operation correlation, and `tracing` emission
- explicit cancellation commands: `CancelSearch`, `CancelScan`, `CancelPreview`,
  and operation-id scoped `CancelOperation`
- `Cancelled` and `TimedOut` error codes for client branching
- provider-aware `Location` primitives: `LocationId`, `LocationDescriptor`,
  `LocationRef`, `LocationRoute`, `LocationSegment`, and `ProviderRef`
- Location-aware navigation, ScanPathCompat, SearchNodeCompat, preview, metadata, cache reuse, and
  refresh paths for direct local providers
- Location-native result events for directory listings, pages, SearchNodeCompat results,
  watcher changes, operation completion, preview results, and metadata results
- directory load contracts through `ListingOptions`, `DirectoryLoadOptions`,
  `DirectoryLoadState`, provider pages, cursors, and page result events
- native `LocalFs` paging and incremental paging for order-preserving filters
- watcher-driven refresh wiring in `FilerCore::with_defaults()`
- write-operation cache invalidation for affected parents and stale directory
  subtrees
- Location-native direct-local watcher and write commands/events
- capability checks for Location-native WatchNodeCompat and write routing
- explicit `*Compat` event variants for legacy `NodeId` and `FileNode` result
  surfaces

Still open:

- canonical command naming for Location-native commands versus path/NodeId
  compatibility commands
- segmented provider/archive execution
- provider profiles and non-local provider routing
- mutation-stable cursor sessions for large directories
- provider-context timeout propagation across provider calls and long-running
  tasks
- extension output envelopes and a first git decoration prototype
- serde/wire protocol envelopes for future server/web transport

## Directory Loading

Directory loading is explicit about cost and result shape.

- `ListingOptions::fast()` uses directory-entry type data and leaves stat-backed
  fields at defaults.
- `ListingOptions::metadata()` stats each entry and fills size, timestamps,
  readonly, and permissions when the provider supports them.
- `DirectoryLoadOptions::default()` requests the first fast page using
  `DEFAULT_DIRECTORY_PAGE_SIZE`.
- `DirectoryLoadOptions::unbounded(listing)` emits a full snapshot.
- `DirectoryLoadOptions::bounded(limit)` emits a trimmed snapshot with
  completeness state.
- `DirectoryLoadMode::Page` emits page events with `DirectoryPageState` and an
  optional `DirectoryCursor`.

`FsProvider::list_page` is the provider paging contract. Providers without
native paging inherit a compatibility fallback that materializes and slices a
full listing. `LocalFs` overrides this and reads only enough entries for the
requested page plus one lookahead entry.

Pipeline paging is supported only when provider order is preserved. Empty
pipelines use provider pages directly. Hidden-file and extension filters run
incrementally over provider pages. Sorting, grouping, size filters, and
name-pattern filters still require full materialization and emit snapshot
events.

Current cursors are best-effort under mutation. If files change between page
requests, offset-backed providers may skip or duplicate rows. Explicit refresh
and watcher-driven refresh are the current recovery paths.

## Cache And Refresh

Directory cache entries are keyed by path and listing detail, so fast and
metadata listings do not mix. Direct local `Location` scans share that storage
through `LocationId` aliases.

Invalidation rules:

- path invalidation removes all listing-detail variants for that path and any
  Location aliases pointing at it
- Location invalidation removes the aliased backing entry
- subtree invalidation removes the exact path, cached descendants, and their
  Location aliases
- create file/folder invalidates the parent
- CopyNodeCompat invalidates the destination parent
- MoveNodeCompat invalidates source and destination parents
- directory MoveNodeCompat, DeleteNodeCompat, and RenameNodeCompat also invalidate the old subtree

Only complete snapshots are cached as complete listings. Partial pages are not
cached as complete directory listings, though later pages may be served from an
existing complete cache entry.

Watcher-driven refresh uses the same invalidation path as manual refresh. In
the default core composition, a provider change under a watched root emits
`FsChanged`, invalidates that root, and refreshes sessions currently displaying
it.

## Request And Operation IDs

`RequestId` tracks async user intent for navigation-driven scans, refresh,
SearchNodeCompat, preview, metadata, and extended metadata. Matching events echo the id.
Actors suppress stale result events when an older request finishes after a newer
one for the same session.

`OperationId` tracks file operations. CopyNodeCompat, MoveNodeCompat, DeleteNodeCompat, RenameNodeCompat, create file,
and create folder echo the operation id on progress, completion, and
operation-scoped errors.

Cancellation is explicit by task family. `CancelSearch`, `CancelScan`, and
`CancelPreview` cancel the active task for a session. `CancelOperation` cancels
only the matching `(session, operation)` pair. `DestroySession` remains the
session-wide cleanup path.

`RequestId::DEFAULT` and `OperationId::DEFAULT` are compatibility placeholders.
New client-originated work should allocate ids with `RequestId::new()`,
`OperationId::new()`, `FilerCore::next_request_id()`, or
`FilerCore::next_operation_id()`.

## Location

`Location` is the additive provider-aware addressing model. It does not remove
the current `PathBuf`, `NodeId`, `FileNode`, or path-based `FsProvider` surfaces
yet.

Use `Location` as the canonical transport identity for new read-side work.
Use `NodeId` as a compatibility/cache handle for existing direct-local flows.

Important types:

- `LocationDescriptor`: reconstructable address data: scheme, provider
  reference, provider-internal root, ordered segments, and optional display path
- `LocationId`: compact id derived from descriptor identity fields; it ignores
  display-only text
- `LocationRef`: transport enum: `Id`, `Descriptor`, or
  `Full { id, descriptor }`
- `LocationSegment`: ordered nested layer, such as an archive member
- `LocationRoute`: routing classification for direct local, segmented, or
  unsupported provider routes
- `NodeEntry`: preferred public row shape for Location-native listing and
  SearchNodeCompat results

Transport rule:

- use `LocationRef::Full` across process, machine, plugin, or storage
  boundaries
- use `LocationRef::Descriptor` when reconstructability matters more than
  compactness
- reserve `LocationRef::Id` for in-process/session-local messages where
  `LocationUnresolved` is recoverable

Direct local Location commands currently route through the existing path-based
execution layer. `Navigate`, `Scan`, `Search`,
`LoadPreview`, `LoadMetadata`, `LoadExtendedMetadata`,
`Watch`, `Unwatch`, and the `*Location` write commands resolve
a `LocationRef`, require a direct path, and preserve the original request and
operation ids on results or structured errors.

Location-native result events use the canonical event names:
`DirectoryLoaded`, `DirectoryPageLoaded`, `SearchResults`, `FsChanged`,
`OperationComplete`, `PreviewReady`, `PreviewFailed`, `MetadataLoaded`, and
`ExtendedMetadataLoaded`. Legacy `NodeId` or `FileNode` result events are
explicit compatibility variants such as `DirectoryLoadedCompat`,
`SearchResultsCompat`, `FsChangedCompat`, and `OperationCompleteCompat`.

Segmented and unsupported-provider routes are represented and reported, but not
executed yet. Nested archives are modeled as a provider root plus ordered
segments, preserving each VFS boundary for later archive/provider traversal.

### NodeId Surfaces

| Label | Surfaces | Contract |
|---|---|---|
| Location-native preferred | `Navigate`, `Scan`, `Search`, `LoadPreview`, `LoadMetadata`, `LoadExtendedMetadata`, `Watch`, `Unwatch`, `*Location` write commands, `DirectoryLoaded`, `DirectoryPageLoaded`, `SearchResults`, `FsChanged`, `OperationComplete`, `PreviewReady`, `PreviewFailed`, `MetadataLoaded`, `ExtendedMetadataLoaded`, `NodeEntry` | Preferred for new provider-aware work. Event names are canonical; command names still carry `Location` until the command cleanup pass. |
| Compatibility | `NavigatePathCompat`, `NavigateNodeCompat`, `SearchNodeCompat`, `SearchPathCompat`, `ScanPathCompat`, `ScanNodeCompat`, `DirectoryLoadedCompat`, `DirectoryPageLoadedCompat`, `SearchResultsCompat`, `PreviewReadyCompat`, `PreviewFailedCompat`, `MetadataLoadedCompat`, `ExtendedMetadataLoadedCompat`, `FileNode` | Supported direct-local/path-era surface. Do not extend with new provider identity semantics. |
| Internal/cache handle | `NodeRegistry`, `NavState.current`, history, selection, direct-local cache bridge ids | Runtime handles for compatibility and cache lookup. |
| Compatibility WatchNodeCompat/write | `WatchNodeCompat`, `UnwatchNodeCompat`, NodeId write commands, `FsChangedCompat`, `OperationCompleteCompat.affected` | Supported direct-local compatibility surface. Prefer Location variants for new provider-aware callers. |

### Capabilities

`LocationWatchCapability` and `LocationOperationCapability` are side-effect-free
checks for Location-native routing. They inspect a `LocationRef`,
`NodeRegistry`, and provider `Capabilities` without starting watches or
mutating files.

Direct local routes use provider `WatchNodeCompat` and `write` booleans. Segmented routes
return `LocationSegmentedUnsupported`; unsupported provider references return
`UnsupportedProvider`; id-only references with no registry entry return
`LocationUnresolved`.

## Progress

Long-running work reports `Event::ProgressUpdated` with a `ProgressScope` and
`ProgressSnapshot`.

- ScanPathCompat progress is request-scoped
- operation progress is request- and operation-scoped
- page completion is described by `DirectoryPageState`
- snapshot completeness is described by `DirectoryLoadState`

## Errors

App-facing errors use:

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

`message` is for display. `code` is the stable machine-readable reason clients
should prefer for behavior. `target` identifies the failed object when core can
name one. `recoverable` is derived from `ErrorCode`.

Create errors through `CoreError` helpers such as `CoreError::not_found(path)`,
`CoreError::permission_denied(path)`, `CoreError::location_unresolved(id)`,
`CoreError::cancelled()`, `CoreError::timed_out(message)`, and
`CoreError::from_io_error(err, path)`.

Use `Event::from_error()`, `Event::from_request_error()`, and
`Event::from_operation_error()` instead of constructing `Event::Error` by hand.
Those helpers preserve correlation and emit structured `tracing` diagnostics.
`filer-core` does not install a global tracing subscriber.

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
    core.send(Command::NavigatePathCompat {
        path: PathBuf::from("/home"),
        session,
        request,
    })
    .unwrap();

    while let Ok(event) = core.event_receiver().recv() {
        if let Event::DirectoryPageLoaded {
            groups,
            page,
            request: loaded_request,
            ..
        } = event
        {
            if loaded_request == request {
                println!("Loaded {} files in this page", groups.total_count);
                if let Some(cursor) = page.next_cursor {
                    println!("More rows are available after {:?}", cursor);
                }
            }
        }
    }
}
```

## Dependencies

- `tokio` - async runtime
- `flume` - actor channels
- `async-trait` - async trait support
