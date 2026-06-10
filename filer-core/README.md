# filer-core

Core library for the Filer file explorer.

Current milestone: `0.3.0`.

## Scope

`filer-core` owns the file-manager kernel:

- sessions and command/event routing
- navigation, scanning, search, watching, preview, and metadata dispatch
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
- versioned protocol envelopes, events, and server transport

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
`Watch`, `Unwatch`, and the Location-native write commands resolve
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
| Location-native preferred | `Navigate`, `Scan`, `Search`, `LoadPreview`, `LoadMetadata`, `LoadExtendedMetadata`, `Watch`, `Unwatch`, `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, `CreateFile`, `DirectoryLoaded`, `DirectoryPageLoaded`, `SearchResults`, `FsChanged`, `OperationComplete`, `PreviewReady`, `PreviewFailed`, `MetadataLoaded`, `ExtendedMetadataLoaded`, `NodeEntry` | Preferred for new provider-aware work. |
| Compatibility | `NavigatePathCompat`, `NavigateNodeCompat`, `SearchNodeCompat`, `SearchPathCompat`, `ScanPathCompat`, `ScanNodeCompat`, `DirectoryLoadedCompat`, `DirectoryPageLoadedCompat`, `SearchResultsCompat`, `PreviewReadyCompat`, `PreviewFailedCompat`, `MetadataLoadedCompat`, `ExtendedMetadataLoadedCompat`, `FileNode` | Supported direct-local/path-era surface. Do not extend with new provider identity semantics. |
| Internal/cache handle | `NodeRegistry`, `NavState.current`, history, selection, direct-local cache bridge ids | Runtime handles for compatibility and cache lookup. |
| Compatibility WatchNodeCompat/write | `WatchNodeCompat`, `UnwatchNodeCompat`, NodeId write commands, `FsChangedCompat`, `OperationCompleteCompat.affected` | Supported direct-local compatibility surface. Prefer Location variants for new provider-aware callers. |

## Command API

Location-native commands use the short canonical Rust names and dispatch keys.
Path and `NodeId` entry points use explicit `*Compat` names and `.compat`
dispatch keys.

| Family | Canonical Rust command | Canonical key | Compatibility Rust command | Compatibility key |
|---|---|---|---|---|
| Navigate | `Navigate` | `navigate` | `NavigatePathCompat`, `NavigateNodeCompat` | `navigate.path.compat`, `navigate.node.compat` |
| Search | `Search` | `search` | `SearchPathCompat`, `SearchNodeCompat` | `search.path.compat`, `search.node.compat` |
| Scan | `Scan` | `scan` | `ScanPathCompat`, `ScanNodeCompat` | `scan.path.compat`, `scan.node.compat` |
| Preview | `LoadPreview` | `preview.load` | `LoadPreviewNodeCompat` | `preview.load.node.compat` |
| Metadata | `LoadMetadata`, `LoadExtendedMetadata` | `metadata.load`, `metadata.extended` | `LoadMetadataNodeCompat`, `LoadExtendedMetadataNodeCompat` | `metadata.load.node.compat`, `metadata.extended.node.compat` |
| Watch | `Watch`, `Unwatch` | `watch`, `watch.remove` | `WatchNodeCompat`, `UnwatchNodeCompat` | `watch.node.compat`, `watch.node.remove.compat` |
| Write | `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, `CreateFile` | `ops.*` | Matching `*NodeCompat` commands | Matching `ops.*.node.compat` keys |

### Rust Migration

No aliases preserve the former Rust variant names. Update callers directly:

| Former command | Current command |
|---|---|
| `Navigate` with `PathBuf` | `NavigatePathCompat` |
| `NavigateLocation` | `Navigate` |
| `NavigateToNode` | `NavigateNodeCompat` |
| `Search`, `SearchPath`, `SearchLocation` | `SearchNodeCompat`, `SearchPathCompat`, `Search` |
| `Scan`, `ScanNode`, `ScanLocation` | `ScanPathCompat`, `ScanNodeCompat`, `Scan` |
| `LoadPreview`, `LoadPreviewLocation` | `LoadPreviewNodeCompat`, `LoadPreview` |
| `LoadMetadata`, `LoadMetadataLocation` | `LoadMetadataNodeCompat`, `LoadMetadata` |
| `LoadExtendedMetadata`, `LoadExtendedMetadataLocation` | `LoadExtendedMetadataNodeCompat`, `LoadExtendedMetadata` |
| `Watch`, `WatchLocation` | `WatchNodeCompat`, `Watch` |
| `Unwatch`, `UnwatchLocation` | `UnwatchNodeCompat`, `Unwatch` |
| Node-based write commands | Matching `*NodeCompat` command |
| `*Location` write commands | Canonical `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, or `CreateFile` |

### Wire DTO

`WireCommand` is the unversioned serde DTO for every built-in command. JSON
uses an internal `type` tag with snake_case labels such as `navigate` and
`navigate_path_compat`. Convert with `Command::from(wire)` and
`WireCommand::try_from(command)`.

`Command::Extension` cannot convert because its `Arc<dyn Any>` payload is
runtime-only. Conversion returns `WireCommandConversionError`. MODULES-001
owns the future wire-safe extension payload. PROTOCOL-001 owns version
envelopes, unknown-field policy, events, and server transport.

### Capabilities

`LocationWatchCapability` and `LocationOperationCapability` are side-effect-free
checks for Location-native routing. They inspect a `LocationRef`,
`NodeRegistry`, and provider `Capabilities` without starting watches or
mutating files.

Direct local routes use provider `watch` and `write` booleans. Segmented routes
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
use filer_core::{Command, Event, FilerCore, Location, LocationRef, RequestId};

#[tokio::main]
async fn main() {
    let core = FilerCore::new().await.unwrap();

    core.send(Command::Handshake).unwrap();
    let session = match core.event_receiver().recv().unwrap() {
        Event::SessionCreated(session) => session,
        other => panic!("expected SessionCreated, got {other:?}"),
    };

    let request = RequestId::new();
    let location = Location::local("/home");
    core.send(Command::Navigate {
        location: LocationRef::from_location(&location),
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
