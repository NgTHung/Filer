# Changelog

This file records what each Filer milestone changed and which contracts it removed. READMEs describe the system as it is. Look here when you need to know when a contract appeared or how to migrate from an older one. Work in progress lives in `.tasks/`, and project direction lives in [ROADMAP.md](ROADMAP.md).

Versions follow the project milestone. Crate versions can lag behind it; check each `Cargo.toml` for the published number.

## 0.3.0, Core contract stabilization (2026-08-31)

This milestone made `LocationRef` the only way to address a file in `filer-core` and removed every compatibility surface from the Location migration.

Added:

- Provider calls carry timeout and capability context into app-facing errors (PROVIDER-001).
- A stable cross-provider directory paging contract (PIPELINE-001).
- Local ZIP archives open as segmented Locations for navigation and scan (VFS-001).
- Operation conflict and undo contracts (OPS-001).
- Hardened actor cancellation, shutdown, and bounded event backpressure (CORE-020).
- A trusted in-process Git decoration prototype that never delays directory loading (MODULES-002).
- Explicit cancellation commands `CancelSearch`, `CancelScan`, `CancelPreview`, and operation-scoped `CancelOperation`, plus the `Cancelled` and `TimedOut` error codes.

Removed:

- Path- and `NodeId`-addressed commands and events (API-006). Their wire tags now fail deserialization as unknown variants.
- The `FileNode` row, row-conversion bridges, and the path-keyed directory cache from the read-side pipeline (API-016).
- `NodeId`-keyed registry maps and compatibility helpers (API-017).
- The `NodeId` type and its deterministic hashing pin (API-008).
- The generic public cancel command, replaced by the per-family commands above.

Fixed:

- `NodeId` hashing panicked on non-UTF-8 paths (CORE-014).
- Directory listing and group ordering used different comparators (CORE-015).
- Cancellation cleanup could clobber a newer request's cancel handle (CORE-016).

Migration: resolve each object to a `LocationRef` before you build a command. The `*Location` write commands are now the canonical `Copy`, `Move`, `Delete`, `Rename`, `CreateFolder`, and `CreateFile`. No aliases preserve the removed names.

## 0.2.4, Location-first read path (2026-05-21)

`Location` became the preferred identity for new read-side work. `NodeId` stayed as a compatibility and cache handle for existing local-path flows while navigation, scan, search, preview, watch, and operation internals moved to `Location`.

## 0.2.3, Location hardening (2026-05-15)

- `LocationRef` gained explicit `Id`, `Descriptor`, and `Full` variants, so an empty reference cannot be constructed.
- `LocationDescriptor` separates the provider root from ordered `LocationSegment` layers.
- `LocationId` hashes the root and ordered segments but ignores display text.
- `LocationRoute` classifies descriptors as direct local, segmented, or unsupported, and the registry caches the derived route.
- Direct local `Location` commands route through navigation, scan, search, preview, metadata, and extended metadata.
- `ListingOptions::fast()` and `ListingOptions::metadata()` let callers choose whether a listing pays for stat calls.
- Provider-level paging through `FsProvider::list_page`, `DirectoryCursor`, `DirectoryPageState`, and `DirectoryPageLoaded`, with `LocalFs` as the first native implementation.
- Hidden-file and extension filters page incrementally instead of loading the whole directory.
- Watcher events for watched roots invalidate navigation cache in the default composition.

Location-native result events took the canonical names. The `PathBuf`, `NodeId`, and `FileNode` result events stayed as explicit compatibility variants until 0.3.0.

## 0.2.2, Location layer

Added the provider-aware `Location` model alongside the existing path and `NodeId` surfaces, without changing public commands or events.

## 0.2.1, Stale-result regression coverage (2026-05-15)

A reliability patch over 0.2.0 with the same public contract. It added regression tests proving that stale scan, search, and preview results are suppressed when a newer request supersedes them, including under parallel test execution.

## 0.2.0, Correlation and error categories (2026-05-15)

- `RequestId` on navigation-driven scans, refresh, search, preview, and metadata, with stale-result suppression.
- `OperationId` on copy, move, delete, rename, create file, and create folder, with operation-scoped progress, completion, and error events.
- Structured `ErrorKind` categories on app-facing errors.
- `filer-app` consumes request ids, operation ids, and error categories.

## 0.1.0 (2026-05-07)

First tagged release of `filer-core`, `filer-app`, and `filer-ecosystem`.
