# Core API Reference

This page describes how `filer-core` behaves at its public boundary: which commands exist, how directory pages and cursors work, when caches drop entries, and how errors reach a client. Read the [crate README](../filer-core/README.md) first for the overall model. Use this page when you build a client or change a contract.

## Commands

Every public command that names a file uses `LocationRef`. The dispatch key routes a command to the module that handles it.

| Family | Rust command | Dispatch key |
|---|---|---|
| Navigate | `Navigate`, `NavigateUp`, `NavigateBack`, `NavigateForward`, `Refresh` | `navigate*` |
| Search | `Search`, `CancelSearch` | `search`, `search.cancel` |
| Scan | `Scan`, `SetPipeline`, `CancelScan` | `scan`, `navigate.pipeline`, `scan.cancel` |
| Preview | `LoadPreview`, `CancelPreview` | `preview.load`, `preview.cancel` |
| Metadata | `LoadMetadata`, `LoadExtendedMetadata` | `metadata.load`, `metadata.extended` |
| Watch | `Watch`, `Unwatch`, `UnwatchSession` | `watch`, `watch.remove`, `watch.session_remove` |
| Write | `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, `CreateFile`, `CancelOperation` | `ops.*` |

Results arrive as `DirectoryLoaded`, `DirectoryPageLoaded`, `SearchResults`, `FsChanged`, `OperationComplete`, `PreviewReady`, `PreviewFailed`, `MetadataLoaded`, and `ExtendedMetadataLoaded`. Listing and search rows use `NodeEntry`.

### Wire format

`WireCommand` is the serde form of every built-in command. JSON uses an internal `type` tag with snake_case labels such as `navigate` and `ops_copy`. Convert with `Command::from(wire)` and `WireCommand::try_from(command)`. An unknown tag fails deserialization instead of reaching a handler.

`Command::Extension` has no wire form, because its `Arc<dyn Any>` payload exists only in process. Converting it returns `WireCommandConversionError`. `WireCommand` carries no version envelope, so both sides of a transport must build from the same revision.

## Request and operation ids

A `RequestId` ties a result to the user action that caused it. Navigation-driven scans, refresh, search, preview, metadata, and extended metadata echo the id on their events. When an older request finishes after a newer one in the same session, core drops the older result, so a slow folder never overwrites the folder the user moved to.

An `OperationId` does the same for file operations. Copy, move, delete, rename, create file, and create folder echo it on progress, completion, and operation-scoped errors.

Allocate ids with `RequestId::new()`, `OperationId::new()`, `FilerCore::next_request_id()`, or `FilerCore::next_operation_id()`. `RequestId::DEFAULT` and `OperationId::DEFAULT` share one value across callers, so results that use them cannot be told apart.

Cancel work by family. `CancelSearch`, `CancelScan`, and `CancelPreview` cancel the active task for a session. `CancelOperation` cancels only the matching session and operation pair. `DestroySession` runs every module's cleanup hook for the session.

## Locations

`Location` addresses a file independently of how a provider stores it. The pieces:

- `LocationDescriptor` holds the reconstructable address: scheme, provider reference, provider root, ordered segments, and an optional display path.
- `LocationId` is a compact hash of the descriptor's identity fields. Display text does not change it.
- `LocationRef` is the transport form: `Id`, `Descriptor`, or `Full { id, descriptor }`.
- `LocationSegment` is one nested layer, such as a member inside an archive.
- `LocationRoute` classifies a descriptor as direct local, segmented, or unsupported.

Choose the `LocationRef` variant by what the receiver can resolve. Use `Full` across process, machine, plugin, or storage boundaries. Use `Descriptor` when the receiver must rebuild the address and size does not matter. Use `Id` only inside one process, where a `LocationUnresolved` error is recoverable because the sender can resend the descriptor.

Direct local routes resolve to a path and run through `LocalFs`. Local ZIP routes support navigation and scan, resolving nested archives in segment order. Other segments and unsupported providers return structured provider errors.

### Capability checks

`LocationWatchCapability` and `LocationOperationCapability` answer whether a watch or write can run on a `LocationRef` without starting it. They read the `NodeRegistry` and the provider's `Capabilities`. Direct local routes follow the provider's `watch` and `write` flags. Segmented routes return `LocationSegmentedUnsupported`, unknown providers return `UnsupportedProvider`, and an id with no registry entry returns `LocationUnresolved`.

## Directory loading

A listing costs what you ask for. Choose the detail and the result shape separately.

- `ListingOptions::fast()` reads directory-entry type data only. Size, timestamps, and permissions stay at defaults.
- `ListingOptions::metadata()` stats each entry and fills size, timestamps, readonly, and permissions when the provider supports them. Request it only when you display those fields.
- `DirectoryLoadOptions::default()` requests the first fast page of `DEFAULT_DIRECTORY_PAGE_SIZE` (256) rows.
- `DirectoryLoadOptions::unbounded(listing)` emits one full snapshot.
- `DirectoryLoadOptions::bounded(limit)` emits a trimmed snapshot with completeness state.
- `DirectoryLoadMode::Page` emits page events with `DirectoryPageState` and an optional `DirectoryCursor`.

`FsProvider::open_listing` returns a `DirectoryStream`, a walk that the scanner can stop and resume. `LocalFs` backs it with a retained directory handle. A provider that returns `None` falls back to `FsProvider::list_page`, whose default implementation loads the full listing and slices it.

### Paging modes

`PipelineConfig::paging_mode` decides how a pipeline pages:

- An empty pipeline emits provider pages directly.
- Hidden-file and extension filters run incrementally over provider pages.
- Sorting and grouping need every row before the first page is correct.
- Size filters and name-pattern filters need a complete snapshot.

The first two stream. A streaming page pulls only its rows plus one lookahead and keeps the walk open, so the first page arrives before the walk reaches the end of the directory, and each continuation costs one page. Streaming modes add no ordering stage, so rows arrive in provider order. Ask for a sort to get a sorted view.

The last two walk the directory once and keep the ordered rows past the returned page, so later pages come from that tail instead of a second walk. One pipeline keeps at most 16,384 rows, and all pipelines share a 32,768-row budget. A pipeline that cannot retain its rows keeps only its keyset boundary and walks again on the next page, which is slower and still correct.

### Cursors

A cursor is opaque and single-use. Core holds at most 256 continuation sessions. When the bound is full, core evicts one. A valid continuation consumes its cursor, and eviction, expiry, or the terminal page releases the provider handle or retained rows. An expired, evicted, or consumed cursor needs a new request without a cursor. Core does not tolerate replay, because keeping consumed state would break the memory bound. Explicit refresh and watcher-driven refresh restart a stale view.

A walked pipeline stores its last ordered row as the boundary. If a sort field changes between page requests, that boundary can skip or duplicate a row. A pipeline serving retained rows answers from the snapshot its walk took, so it does not see later changes.

`DirectoryPageState::total_count` is `None` while a streaming pipeline is partial, because a stopped walk cannot count rows it has not seen. The terminal page carries the final count. A walked pipeline reports the count from its first page and reuses it for continuations, so treat it as a point-in-time estimate.

## Cache and refresh

The directory cache keys entries by `LocationId` and listing detail, so fast and metadata listings never mix. Only complete snapshots enter the cache. Later pages can be served from an existing complete entry.

Writes invalidate what they can change:

- Invalidating a Location removes every listing-detail variant for it.
- Local subtree invalidation removes the exact path and every cached descendant.
- Create file and create folder invalidate the parent.
- Copy invalidates the destination parent.
- Move invalidates the source and destination parents.
- Directory move, delete, and rename also invalidate the old subtree.

Watcher refresh takes the same path as manual refresh. In `FilerCore::with_defaults()`, a change under a watched root emits `FsChanged`, invalidates that root, and refreshes every session displaying it.

## Progress

Long-running work reports `Event::ProgressUpdated` with a `ProgressScope` and a `ProgressSnapshot`. Scan progress is scoped to a request, and operation progress to a request and operation. `DirectoryPageState` reports page completion, and `DirectoryLoadState` reports snapshot completeness.

## Errors

Errors reach clients as `Event::Error`:

```rust
Event::Error {
    kind,
    code,
    target,
    context,
    message,
    recoverable,
    session,
    request,
    operation,
}
```

Branch on `code`, which is stable. Show `message` to the user, but do not parse it. `target` names the failed object when core knows it, and `context` carries structured detail, such as the provider that timed out or the source and destination of a write collision. Core derives `recoverable` from `code`.

Build errors with `CoreError` helpers such as `CoreError::not_found(path)`, `CoreError::permission_denied(path)`, `CoreError::location_unresolved(id)`, `CoreError::cancelled()`, `CoreError::timed_out(message)`, and `CoreError::from_io_error(err, path)`. Emit them with `Event::from_error()`, `Event::from_request_error()`, or `Event::from_operation_error()`. These helpers keep the request and operation correlation and emit a structured `tracing` event. `filer-core` never installs a tracing subscriber, so the application decides where diagnostics go.
