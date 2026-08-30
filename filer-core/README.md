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
| `actors/` | Shared actor infrastructure |
| `modules/` | Navigation, scan, search, watch, preview, operation, and extension workers |
| `vfs/` | Provider contracts, local filesystem access, watching, and segmented archive routing |
| `pipeline/` | Filter, sort, group, and paging policy |
| `services/` | MIME, metadata, preview, and cache services |
| `utils/` | Shared helpers |

## Current Contracts

The active stabilization work is provider-aware core behavior before a larger
app rewrite or full extension runtime.

Completed:

- request ids and stale-result guards for scan, search, preview, metadata, and
  refresh flows
- operation ids for copy, move, delete, rename, create file, and create folder
- structured app-facing errors with stable `ErrorCode`, optional `ErrorTarget`,
  request/operation correlation, and `tracing` emission
- explicit cancellation commands: `CancelSearch`, `CancelScan`, `CancelPreview`,
  and operation-id scoped `CancelOperation`
- `Cancelled` and `TimedOut` error codes for client branching
- provider-aware `Location` primitives: `LocationId`, `LocationDescriptor`,
  `LocationRef`, `LocationRoute`, `LocationSegment`, and `ProviderRef`
- Location-aware navigation, scan, search, preview, metadata, cache reuse, and
  refresh paths for direct local providers
- Location-native result events for directory listings, pages, search results,
  watcher changes, operation completion, preview results, and metadata results
- directory load contracts through `ListingOptions`, `DirectoryLoadOptions`,
  `DirectoryLoadState`, provider pages, cursors, and page result events
- native `LocalFs` paging and incremental paging for order-preserving filters
- watcher-driven refresh wiring in `FilerCore::with_defaults()`
- write-operation cache invalidation for affected parents and stale directory
  subtrees
- Location-native direct-local watcher and write commands/events
- capability checks for Location-native watch and write routing
- removal of path- and `NodeId`-addressed compatibility commands and events

Still open:

- provider registry and profile-backed provider resolution
- provider profiles and non-local provider routing
- mutation-stable cursor sessions for large directories
- extension output envelopes and a first git decoration prototype
- versioned protocol envelopes, events, and server transport

Deferred:

- concrete S3, WebDAV, SFTP, encrypted, Kubernetes, sync, and cloud-placeholder providers
- OS mount adapters such as FUSE or WinFsp
- non-archive virtual segment execution beyond the current structured errors

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

## Large Directory Benchmark

Run the public command-path benchmark with:

```bash
cargo bench -p filer-core --bench large_directory
```

The runner generates 10,000 local entries, excludes fixture creation from the
timed samples, and reports minimum, median, p95, maximum, and mean latency. It
measures a fast first page, fast next page, metadata first page, sorted first
page, and fast full snapshot. Use the same machine, filesystem, Rust toolchain,
entry count, and page size when you compare revisions.

By default, the fixture uses your temporary directory. Set
`FILER_BENCH_FIXTURE_ROOT` to measure a specific filesystem. You can also set
`FILER_BENCH_ENTRIES`, `FILER_BENCH_PAGE_SIZE`, `FILER_BENCH_SAMPLES`, and
`FILER_BENCH_WARMUP` to run a different profile. Keep the defaults for the
recorded 10,000-entry baseline.

The provisional bounded-work gate is a separate structural test:

```bash
cargo test -p filer-core --test large_directory_paging_test -- --ignored
```

This test is ignored in the normal suite while the gate fails. It counts rows
returned by a native provider through the public scan command, so a fast machine
cannot hide a full directory walk. Recorded results and machine details live in
[`benches/baselines/`](benches/baselines/).

## Cache And Refresh

Directory cache entries are keyed by `LocationId` and listing detail, so fast
and metadata listings do not mix. Each entry retains its stored `Location`
and provider-owned `NodeEntry` rows.

Invalidation rules:

- Location invalidation removes all listing-detail variants for that location
- local subtree invalidation removes the exact path and cached local descendants
- create file/folder invalidates the parent
- copy invalidates the destination parent
- move invalidates source and destination parents
- directory move, delete, and rename also invalidate the old subtree

Only complete snapshots are cached as complete listings. Partial pages are not
cached as complete directory listings, though later pages may be served from an
existing complete cache entry.

Watcher-driven refresh uses the same invalidation path as manual refresh. In
the default core composition, a provider change under a watched root emits
`FsChanged`, invalidates that root, and refreshes sessions currently displaying
it.

## Request And Operation IDs

`RequestId` tracks async user intent for navigation-driven scans, refresh,
search, preview, metadata, and extended metadata. Matching events echo the id.
Actors suppress stale result events when an older request finishes after a newer
one for the same session.

`OperationId` tracks file operations. Copy, move, delete, rename, create file,
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

`Location` is the provider-aware addressing model. API-016 removes the
`FileNode` row, row-conversion bridges, and path-keyed directory cache from
the read-side pipeline. API-006 removes the path- and `NodeId`-addressed
compatibility routes from the public command, wire, and event surfaces.
API-017 removes the `NodeId`-keyed registry maps and compatibility helpers.

Use `Location` as the canonical transport identity for new read-side work.
The `NodeId` type and deterministic hashing pin remain only until API-008
deletes them. No runtime registry, navigation, or workflow depends on them.

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
  search results

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
`ExtendedMetadataLoaded`. These are now the only public result event variants;
the former NodeId/FileNode compatibility events were removed by API-006.

Local ZIP segmented routes execute for navigation and scan. Nested archives are
resolved in descriptor segment order, and listed archive members carry display,
target, read, and navigation metadata. Non-archive segments, virtual segments,
and unsupported providers return structured provider errors. Concrete remote
providers, encrypted providers, Kubernetes, sync, cloud-placeholder providers,
and OS mount adapters are not part of the current `filer-core` provider surface.

### Identity Surfaces

| Label | Surfaces | Contract |
|---|---|---|
| Public Location-native | `Navigate`, `Scan`, `Search`, `LoadPreview`, `LoadMetadata`, `LoadExtendedMetadata`, `Watch`, `Unwatch`, `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, `CreateFile`, `DirectoryLoaded`, `DirectoryPageLoaded`, `SearchResults`, `FsChanged`, `OperationComplete`, `PreviewReady`, `PreviewFailed`, `MetadataLoaded`, `ExtendedMetadataLoaded`, `NodeEntry` | The only public addressing and result contract. |
| Internal Location registry | `NodeRegistry`, `LocationId`, `LocationRef::Id` resolution | Stores descriptors and route cache entries; it has no NodeId-keyed state. |
| API-008 cleanup pin | `NodeId` definition and deterministic hashing test | Temporary compatibility surface with no runtime callers; API-008 owns its deletion. |
| Removed by API-006 | Former path- and NodeId-addressed commands and events | Legacy wire tags fail deserialization as unknown variants. |

## Command API

Location-native commands use the short canonical Rust names and dispatch keys.
All built-in public commands use `LocationRef` where they address filesystem
objects.

| Family | Rust command | Dispatch key |
|---|---|---|
| Navigate | `Navigate`, `NavigateUp`, `NavigateBack`, `NavigateForward`, `Refresh` | `navigate*` |
| Search | `Search`, `CancelSearch` | `search`, `search.cancel` |
| Scan | `Scan`, `SetPipeline`, `CancelScan` | `scan`, `navigate.pipeline`, `scan.cancel` |
| Preview | `LoadPreview`, `CancelPreview` | `preview.load`, `preview.cancel` |
| Metadata | `LoadMetadata`, `LoadExtendedMetadata` | `metadata.load`, `metadata.extended` |
| Watch | `Watch`, `Unwatch`, `UnwatchSession` | `watch`, `watch.remove`, `watch.session_remove` |
| Write | `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, `CreateFile`, `CancelOperation` | `ops.*` |

### Rust Migration

No aliases preserve the removed Rust variant names. Update callers to construct
`LocationRef` values and use the canonical commands directly.

| Former surface | Migration |
|---|---|
| Path- or NodeId-addressed navigation, search, scan, preview, metadata, watch, or operation commands | Resolve the object to a `LocationRef` before constructing the canonical command. |
| `*Location` write commands | Canonical `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, or `CreateFile` |

### Wire DTO

`WireCommand` is the unversioned serde DTO for every built-in command. JSON
uses an internal `type` tag with snake_case labels such as `navigate` and
`ops_copy`. Convert with `Command::from(wire)` and
`WireCommand::try_from(command)`.

The former path- and NodeId-addressed wire tags are absent. Deserializing one
returns an unknown-variant error instead of routing to a compatibility handler.

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

- scan progress is request-scoped
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
