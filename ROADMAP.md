# Filer — Engineering Roadmap

> **Development model:** Test-driven. Tests are written before implementation for every non-trivial item.
> A phase is **complete** when: all tests pass, `cargo clippy` is clean, and public API is documented.

---

## Architecture & Design Invariants

These constraints are not negotiable. Any phase that violates them requires a design review.

**`filer-core` is a library, not an application.**
Every frontend — desktop, web, mobile — is a thin consumer of the same core. The core never imports GUI, HTTP, or platform-specific crates. Features are gated. Frontends are in separate crates.

**All I/O goes through `FsProvider`.**
Extractors, previewers, and actors never call `std::fs`, `tokio::fs`, or any I/O crate directly. The `FsProvider` abstraction is the seam that makes remote backends (S3, WebDAV, SFTP) and virtual providers (archives, vaults) transparent to all consumers. Violating this couples business logic to the local filesystem and cannot be tested without real files.

**Actors own their state exclusively.**
Actors communicate only via `flume` channels. No actor field is accessed from outside the actor's own `run()` loop — the only shared data between actors and spawned tasks is `Arc`-wrapped infrastructure (`CancelMap`, `NodeRegistry`, `Arc<dyn FsProvider>`). This prevents data races without `Mutex` on hot paths.

**Sessions are the unit of isolation.**
Every command and event carries a `SessionId`. A UI instance may open multiple sessions (split-pane, tabs). No session leaks state into another. `SessionManager` is the only source of truth for session lifecycle; actors clean up per-session state in `on_session_destroy` hooks.

**The pipeline is the only transformation path.**
`GroupedNodes` is the sole output type for directory listings. Filters, sorts, and groupings are applied by composing `Stage` implementations in `Pipeline`. Actors do not sort or filter inline — they hand raw `Vec<FileNode>` to the pipeline. This keeps transformation logic testable without actors.

**Feature flags are an architecture concern, not a convenience.**
Each optional capability is behind a named feature that compiles to zero overhead when disabled. Every extractor, provider, and heavy-weight dependency must be gated. The base `filer-core` (no features) must compile and all core tests must pass in under 30 seconds.

---

## Definition of Done

A phase is **done** when:
1. All listed test cases are implemented and passing.
2. No `todo!()`, `unimplemented!()`, or `panic!()` in the code path (stubs in unreachable branches are acceptable).
3. `cargo clippy -p filer-core` produces zero warnings on the changed code.
4. Public items have doc comments explaining purpose and invariants.
5. Feature-gated code is verified with `--features` and without.

---

## ✅ Milestone 0 — Core Engine (Complete)

*The following phases are fully implemented and tested. Documented here as a stable baseline.*

### Phase 1: Foundation
- [x] `CoreError` — typed error enum with `from_io_error(err, path)`, display, and recoverable classification
- [x] `NodeId` — deterministic, content-addressed identifier (path hash); cheaply copied, not interned
- [x] `SessionId` — UUID-backed, zero-copy clone
- [x] `FileNode` / `NodeMeta` — filesystem entry representation with lazy owner/group resolution
- [x] `NodeRegistry` — `Arc<scc::HashMap>` mapping `NodeId ↔ PathBuf`; shared across actors without lock contention
- [x] `FsProvider` trait — read-only interface: `list`, `read`, `read_range`, `read_header`, `open_reader`, `exists`, `metadata`
- [x] `LocalFs` — production implementation; `list` uses `tokio::fs::read_dir`, skips unreadable entries with `tracing::debug`; `open_reader` returns `BufReader<std::fs::File>` (zero extra allocation)
- [x] `Command` / `Event` enums — full set of variants with `SessionId` tagging; `Command::Extension` for plugin extensibility without enum modification
- [x] `Capabilities` — per-provider capability advertisement (`read`, `write`, `watch`, `search`)

### Phase 2: Pipeline System
- [x] `Stage` trait — `fn execute(nodes: Vec<FileNode>) -> Vec<FileNode>`; pure function, no I/O
- [x] `FilterHidden`, `FilterByExtension` — filter stages
- [x] `SortBy` — sort by name/size/date, ascending/descending, optional directories-first
- [x] `GroupBy` — group by extension/date/size; degenerate single-group output maintains uniform `GroupedNodes` contract
- [x] `Pipeline` — composes stages; `execute_grouped` is the sole output path; always returns `GroupedNodes`
- [x] `PipelineConfig` — serializable, sent from UI to core; core builds `Pipeline` from it

### Phase 3: Actor Infrastructure
- [x] `Actor` trait — `async fn run(self)`, consumed on spawn; `name()` for tracing
- [x] `ActorSystem` — tracks `JoinHandle`s; `shutdown()` aborts all tasks, cascading channel closure
- [x] `CancellationToken` + `CancelMap` — shared atomic-flag cancellation; `arm(session)` cancels previous in-flight task and issues a fresh token; `cancel_all()` used on shutdown; no actor reimplements this
- [x] `Scanner` — directory traversal: `list → register → pipeline → emit DirectoryLoaded`; cancellable at two yield points (post-list, post-pipeline)
- [x] `Navigator` — history stack with bounded depth; `NavState` snapshot emitted on every transition
- [x] `CommandRouter` — string-keyed dispatch via `HandlerRegistry`; `Command::Extension` routes by user-provided key

### Phase 4: FilerCore API
- [x] `SessionManager` — issues `SessionId`, routes events per session, cleans up on destroy
- [x] `Module` trait + `ModuleContext` — composable initialization; modules register handlers and spawn actors; swappable without modifying core
- [x] `HandlerRegistry` — `scc::HashMap`-backed; thread-safe registration and dispatch; `on_session_destroy` hook chain
- [x] `FilerCore` — public entry point; `send(command)`, `event_receiver()`, `load(module)`, `with_defaults()`
- [x] Navigation flow integration tests — covers full round-trip from `Command` to `Event`

### Phase 5: File Watching
- [x] `Watcher` actor — `notify`-based; debounces rapid filesystem events (configurable window); emits `FsChanged` per `NodeId` with `FsChangeKind`
- [x] Per-session and per-`NodeId` watch/unwatch; session destroy automatically unregisters all watches

### Phase 6: Search
- [x] `SearchQuery` parser — text, glob (`*.rs`, `test?.txt`), `size:>1mb`, `modified:<1w`, `type:image`; combined filters
- [x] `Searcher` actor — recursive BFS with configurable depth limit; batched streaming results (`SearchResults { complete: false }` → `{ complete: true }`); cancellable per session; honors `max_results`

### Phase 7: MIME & Metadata
- [x] `MimeDetector` — static, three-tier: `EXT_TABLE` binary search → `infer` magic bytes → `new_mime_guess` fallback; `Definitive` confidence short-circuits magic-byte I/O; ambiguous extensions (`bin`, `dat`, `tmp`, …) never hit `EXT_TABLE`
- [x] `MetadataExtractor` trait + `MetadataRegistry` — routes by `MimeCategory`; feature-gate pattern: `#[cfg(not(feature = "..."))] return Ok(Unavailable);`
- [x] `ImageExtractor` — `imagesize` for dimensions; `kamadak-exif` for EXIF (orientation, GPS, camera model, color space, bit depth) [`metadata-image`]
- [x] `AudioExtractor` — `id3` for tags (title, artist, album, genre, year) and duration [`metadata-audio`]
- [x] `VideoExtractor` — `mp4parse` for dimensions, duration, video/audio codec; buffers via `provider.read()` + `Cursor` to satisfy `T: Sized` bound [`metadata-video`]
- [x] `DocumentExtractor` — `lopdf::Document::load_metadata_from` for PDF page count, title, author, dates [`metadata-document`]
- [x] `ArchiveExtractor` — ZIP/TAR/GZ/BZ2/XZ/ZSTD/7Z entry listing and size aggregation; RAR behind separate `metadata-archive-rar` feature (C++ dep, RARlab license) [`metadata-archive`]

### Phase 8: Shared Infrastructure
- [x] `PathUtils`, `SizeUtils`, `TimeUtils` — formatting helpers, no I/O
- [x] `CancellationToken` + `CancelMap` — extracted from actors into `actors/cancel.rs`; single implementation, three consumers (Scanner, Searcher, Previewer)

---

## 🚧 Milestone 1 — MVP Desktop App

> **Goal:** A fully functional, locally-operated file manager with no `todo!()` on any reachable code path.
> This milestone ends when Phase 13 ships a working Iced application.

### Phase 9: Write Operations — `FsProvider` Extension

> **Blocker:** `FsProvider` currently has no write surface. `Operator` cannot be implemented without it.
> This is a breaking change to the trait — all provider implementations (including stubs) must be updated.

**Design constraint:** Write methods must be added as non-defaulted `async fn` on `FsProvider`. Providers that advertise `capabilities().write = false` may return `CoreError::PermissionDenied` from write methods, but they must compile. Remote providers that support writes (S3 put, WebDAV PUT) implement these naturally.

- [ ] `write(path, data: &[u8])` — create or overwrite; respects parent existence
- [ ] `copy(src, dst)` — provider-local optimized copy; falls back to read+write across providers
- [ ] `rename(src, dst)` — atomic on same filesystem; errors on cross-device without fallback
- [ ] `delete(path, trash: bool)` — trash uses OS facilities (`trash` crate); permanent is recursive for directories
- [ ] `mkdir(path)` — creates intermediate directories (`mkdir -p` semantics)
- [ ] `LocalFs` implementation for all five methods, including cross-platform trash support
- [ ] Update all stub providers (`ArchiveFs`, `S3Fs`, `WebDavFs`, etc.) to compile with new methods
- [ ] Tests: each write operation on `LocalFs` using `tempfile::TempDir`; verify idempotency where applicable

### Phase 10: File Operations Actor

> **Depends on:** Phase 9. `Operator` struct and `OpsCommand` enum are scaffolded; all logic is `todo!()`.

**Design notes:** Long-running operations (recursive copy, large delete) must be chunked and yield-point-cancellable. Progress granularity is per-file, not per-byte, to avoid channel saturation. Cross-filesystem move is copy-then-delete, not atomic; the event sequence must reflect this (`OperationComplete` only after delete succeeds). The `Operator` must use `CancelMap` — one cancel token per session per operation.

- [ ] Tests: `Copy` single file → `OperationProgress` events + `OperationComplete { success: true }`
- [ ] Tests: `Copy` directory recursively → progress reflects file count; cancel mid-flight leaves partial copy
- [ ] Tests: `Move` same-filesystem → single rename, no progress events, atomic
- [ ] Tests: `Move` cross-filesystem → copy-then-delete sequence, progress emitted
- [ ] Tests: `Delete { trash: false }` → file removed; `{ trash: true }` → file in OS trash
- [ ] Tests: `Rename` file and directory
- [ ] Tests: `CreateFolder` and `CreateFile` with name collision → `CoreError::AlreadyExists`
- [ ] Tests: cancel in-flight copy → partial state is cleaned up
- [ ] `Operator` implementation — uses `CancelMap`, emits `OperationProgress`, calls `provider` write methods
- [ ] Wire `OperationsModule` into `FilerCore::with_defaults()`

### Phase 11: Preview Providers

> **Context:** `PreviewRegistry`, `PreviewCache`, and `Previewer` actor are implemented.
> Zero tests exist for any of these. Five providers are stubbed with `todo!()`.

**Design constraint:** Providers receive a `&Path` and `&PreviewOptions`, not a `&dyn FsProvider`. This is an intentional simplification for the preview tier — previews are generated on local files only for now. Remote preview will route through a local cache file. The `PreviewData` enum must not grow unboundedly; new variants require a design decision on the `serde` boundary.

**Performance budget:** Text preview of a 1 MB file must complete under 5 ms. Image thumbnail generation (1920×1080 → 256×256) must complete under 50 ms. These are not tested automatically but should be validated manually before shipping.

- [ ] Tests: `PreviewRegistry::get_provider` returns highest-priority provider for each `MimeCategory`
- [ ] Tests: `PreviewRegistry::can_preview` returns false for unsupported MIME types
- [ ] Tests: `PreviewCache::put` / `get` / TTL expiry / size-based eviction
- [ ] Tests: `Previewer` actor — cache hit skips generation; cancel mid-generation emits nothing; `ClearCache` resets state
- [ ] `TextProvider` — `provider.read()` → truncate to `max_bytes`; emit `PreviewData::Text { content, truncated }`
- [ ] `CodeProvider` — language detection from extension; syntax highlighting via `syntect` [`preview-code` feature]
- [ ] `ImageProvider` — thumbnail via `image` crate, aspect-ratio preserving; emit `PreviewData::Image { data, width, height, original_width, original_height }` [`preview-image` feature]
- [ ] `MediaProvider` — emit metadata-derived summary (duration, codec); waveform generation deferred
- [ ] `ArchiveProvider` — reuse `ArchiveExtractor` output; emit entry listing as `PreviewData::Archive`

### Phase 12: Directory Cache

> **Depends on:** Phase 10. File operations are the primary cache invalidation trigger.

**Design notes:** The cache is a service, not an actor — it lives as an `Arc<Mutex<DirCache>>` shared between `Scanner` and `OperationsModule`. An actor adds unnecessary latency on the hot scan path. Invalidation must be conservative: any write operation on a path invalidates its parent directory entry. The cache must have a hard memory ceiling (configurable, default 128 MB) with LRU eviction.

- [ ] Tests: `DirCache::get` returns `None` on first access, cached result on second
- [ ] Tests: `FsChanged` event invalidates the correct directory entry
- [ ] Tests: write operation (copy/move/delete/create) invalidates parent
- [ ] Tests: LRU eviction respects size ceiling; oldest entry evicted first
- [ ] `DirCache` implementation — size-bounded LRU, `scc::HashMap`, eviction on insert when over limit
- [ ] `Scanner` integration — check cache before `provider.list()`; update cache on successful list

### Phase 13: Iced Desktop GUI

> **Milestone gate.** This phase is what makes everything above visible to a user.

**Architecture note:** The GUI crate (`filer-app` or `filer-iced`) imports `filer-core` as a library dependency. All application state derives from `Event` emissions. The GUI never calls filesystem APIs directly. The `FilerCore` instance lives in a `tokio` runtime; Iced subscribes to the event channel via its `Subscription` mechanism.

- [ ] **Application shell** — main window, `App` state machine, `Message` type, runtime bootstrap
- [ ] **Core subscription** — `Subscription` polls `FilerCore::event_receiver()`; dispatches `Event → Message → update()`
- [ ] **File list view** — name, size (formatted), modified date, MIME icon; virtualized for directories > 500 entries
- [ ] **Sidebar** — places (home, documents, downloads, desktop, mounted drives); bookmarks (persistent, user-managed)
- [ ] **Breadcrumb bar** — clickable path segments derived from `NavState`
- [ ] **Preview panel** — renders `PreviewData` variants: text (monospace, scrollable), image (scaled), metadata table, archive entry list
- [ ] **Search bar** — debounced input (150 ms); live `SearchResults` streaming into file list view
- [ ] **Status bar** — item count, selection count, selection total size, current sort
- [ ] **Keyboard navigation** — arrows, `Enter` (open), `Backspace` (up), `Alt+Left/Right` (back/forward), `/` (focus search)
- [ ] **Context menu** — copy, cut, paste, delete, rename, properties; multi-selection aware
- [ ] **Drag and drop** — within the app triggers `Move`; from external apps triggers `Copy`
- [ ] **Thumbnail loading** — lazy, off main thread via `Previewer`; placeholder rendered until ready
- [ ] **Thumbnail disk cache** — persist thumbnails to `$XDG_CACHE_HOME/filer/thumbs` keyed by content hash

---

## 🌐 Milestone 2 — Web Application

> **Goal:** Run filer in any browser. The core is already multi-session; the only new layer is transport.
> The architecture here must not couple `filer-core` to HTTP or WebSocket crates.
> Transport lives in a thin `filer-server` crate that depends on `filer-core`, not vice versa.

### Phase 14: Serialization Boundary

> **Prerequisite for Milestone 2.** `Command` and `Event` need a stable wire format.

**Design decision:** Use `serde` with a versioned JSON envelope (`{ "v": 1, "type": "Navigate", "payload": {...} }`). This envelope allows protocol evolution without breaking existing clients. `NodeId` and `SessionId` serialize as UUIDs/hex strings, not internal representations.

- [ ] `serde` derives for `Command`, `Event`, `NodeId`, `SessionId`, `FileNode`, `NodeMeta`, `GroupedNodes`, `PreviewData`, `ExtendedMetadata`
- [ ] Tests: `Command → JSON → Command` roundtrip for all variants
- [ ] Tests: `Event → JSON → Event` roundtrip for all variants
- [ ] Tests: unknown fields in JSON are ignored (forward compatibility)
- [ ] Versioned envelope — `{ "v": 1, "id": "<request-id>", "payload": {...} }`

### Phase 15: WebSocket Server (`filer-server`)

- [ ] `axum` or `tokio-tungstenite` WebSocket server
- [ ] Connection lifecycle: connect → `Command::Handshake` → `SessionCreated`; disconnect → `Command::DestroySession`
- [ ] Request/response correlation — client attaches `id` to commands; server echoes `id` in the corresponding response event
- [ ] Event streaming — all `Event`s for the session are pushed over the same connection
- [ ] Tests: connect → navigate → receive `DirectoryLoaded`; disconnect → session destroyed
- [ ] Tests: two concurrent connections receive independent event streams

### Phase 16: WASM Client Library (`filer-web`)

- [ ] Shared types compiled to WASM via `wasm-bindgen`
- [ ] `WebSocketClient` — async event subscription, typed send/receive
- [ ] Reconnection with exponential back-off
- [ ] TypeScript type generation from WASM bindings

### Phase 17: Web UI

- [ ] Framework evaluation: Leptos (fine-grained reactivity, SSR-capable) preferred over Dioxus/Yew based on ecosystem maturity at time of implementation
- [ ] Core integration via `filer-web` WebSocket client
- [ ] Parity with Phase 13 desktop views: file list, sidebar, breadcrumb, search, preview panel

---

## 🔧 Milestone 3 — Power Features

> These address the extensibility that distinguishes filer from a basic file browser.

### Phase 18: Archive VFS Provider

> `ArchiveFs` stub exists. Implementing this closes the loop on the VFS abstraction — a user should be able to navigate into a ZIP as if it were a directory, using the same `Navigate` command and `DirectoryLoaded` event.

**Design constraint:** `ArchiveFs` is read-only (`capabilities().write = false`). It operates on a single archive file opened through an outer `FsProvider` — it must not open the archive path directly. Seeking is required; the outer provider must support `open_reader()`.

- [ ] Tests: `ArchiveFs::list("/")` on a ZIP returns correct `FileNode` tree
- [ ] Tests: `ArchiveFs::read("inner/file.txt")` returns correct bytes
- [ ] Tests: `ArchiveFs::list` on a nested directory within a TAR
- [ ] `ZipFs` — uses `zip` crate; builds in-memory directory tree on open; `list` and `read` are O(1) lookups
- [ ] `TarFs` — streams TAR to build index on open; supports gz/bz2/xz/zstd via compression layer
- [ ] Integration: `FilerCore` detects archive MIME on `Navigate(path)`; swaps provider to `ArchiveFs`; emits `DirectoryLoaded` with archive contents

### Phase 19: Remote VFS Backends (feature-gated)

> Each backend is a separate feature and a separate crate dependency. None affect compile time when disabled.
> All must implement `FsProvider` in full, including write methods where the protocol supports them.

- [ ] **S3** (`s3`) — `aws-sdk-s3`; `list` maps to `ListObjectsV2`; `read` to `GetObject`; `write` to `PutObject`; `delete` to `DeleteObject`; `rename` is copy+delete (no atomic rename in S3)
- [ ] **WebDAV** (`webdav`) — `PROPFIND` for `list`/`metadata`; `GET` for `read`; `PUT` for `write`; `MOVE` for `rename`; `DELETE` for delete; `MKCOL` for `mkdir`
- [ ] **SFTP** (`sftp`) — `ssh2` crate; connection pooling (one connection per provider instance); all standard operations
- [ ] **FUSE** (`fuse`) — mounts any `FsProvider` as a FUSE filesystem; allows shell and other apps to browse remote/virtual providers
- [ ] **Kubernetes** (`k8s`) — `kube` crate; namespaces → pods → containers → files; read-only; `read` fetches resource YAML via API

> **API stability note:** Each backend's config struct (`S3Config`, `WebDavConfig`, etc.) is part of the public API from the moment it ships. Design these carefully — field additions are non-breaking, field removals or type changes are.

### Phase 20: Encryption / Vault (`crypto` feature)

> Cipher primitives (`AES-256-GCM`, `XChaCha20-Poly1305`) and key derivation (`Argon2id`) are scaffolded.
> `Vault` is a `FsProvider` decorator — it wraps any other provider with transparent encryption.
> This means an encrypted vault can sit on S3, WebDAV, or the local filesystem without code changes.

- [ ] Tests: encrypt/decrypt roundtrip — plaintext in, ciphertext stored, plaintext out
- [ ] Tests: wrong password fails with `CoreError::PermissionDenied`, not a panic or data corruption
- [ ] Tests: `Vault::list` decrypts filenames; `Vault::read` decrypts content
- [ ] Tests: `Vault::change_password` — re-encrypts the master key, not every file
- [ ] `Vault` implementation — wraps `Box<dyn FsProvider>`; master key encrypted with Argon2-derived KEK; per-file nonces; authenticated encryption prevents silent corruption
- [ ] Filename encryption — encrypted filenames are base64url-encoded to remain valid path components

---

## 🔌 Milestone 4 — Ecosystem

> These make filer a platform. Do not start this milestone until Milestone 2 is complete.
> Ecosystem features live or die by external adoption — design their APIs to be stable and versioned from day one.

### Phase 21: Plugin System

> `Command::Extension` and `Event` already provide the extension seam. Plugins are `Module` implementations loaded at runtime or compiled in as features.

**Design constraint:** Plugins must declare their capabilities upfront (which command keys they handle, which event types they emit). This allows the core to reject conflicting plugins at load time and enables UI to discover plugin capabilities without running them.

- [ ] Plugin manifest — `name`, `version`, `commands: Vec<&str>`, `events: Vec<&str>`, `requires: Vec<Capability>`
- [ ] `PluginRegistry` — validates manifests, detects conflicts, loads in dependency order
- [ ] Dynamic loading path — `libloading` for `.so`/`.dll` plugins (Linux/Windows)
- [ ] Static plugin path — plugins compiled in via feature flags (zero overhead, no `unsafe`)
- [ ] Sandboxing — plugins that handle I/O must receive a scoped `FsProvider`; they cannot access the global registry
- [ ] Example: `git-status` plugin — overlays git status on `FileNode`s using `git2`
- [ ] Example: `image-optimizer` plugin — registers `ops.optimize` command key; bulk re-encodes images

### Phase 22: Themes & Accessibility

- [ ] `Theme` struct — color tokens, spacing scale, typography; serializable to/from TOML
- [ ] Icon pack interface — maps `MimeCategory` + `FsChangeKind` to icon identifiers; swappable at runtime
- [ ] Dark/light mode — follows OS preference (`dark-light` crate); manual override persisted
- [ ] Keyboard shortcut remapping — `keybindings.toml`; validated on load (no duplicate bindings)
- [ ] Accessibility metadata — `FileNode` carries enough information for screen readers; no I/O in accessibility paths

### Phase 23: Sync & Backup

- [ ] `SyncEngine` — compares two `FsProvider` instances by path + content hash; produces `SyncPlan` (additions, deletions, conflicts)
- [ ] Conflict strategies — `newer-wins`, `larger-wins`, `manual` (emits `SyncConflict` event for UI resolution)
- [ ] Incremental backup — snapshot-based; only changed files since last snapshot are written
- [ ] Versioning — N retained versions per file; configurable per-vault

---

## 📱 Future Consideration

### Phase 24: Mobile

> Do not plan this in detail until Milestone 2 (web) is complete. The web transport layer may serve as the mobile backend via a local loopback server, avoiding a separate FFI bridge.

- [ ] Evaluate: Tauri Mobile (shares Rust core directly) vs. WebSocket-over-loopback (shares web UI)
- [ ] Touch interaction model (swipe-to-delete, long-press context menu, pinch-zoom in preview)
- [ ] Adaptive layout: phone (single-pane) / tablet (split-pane)
- [ ] Background indexing on mobile requires explicit battery/wake-lock management

---

## Technical Debt Register

*Known issues that do not block any current milestone but should be resolved before the item upstream of them ships.*

| Issue | Impacts | Priority |
|-------|---------|----------|
| `FsProvider` has no write methods; `Operator` stubs all logic | Phase 10 blocked | **High — Phase 9** |
| `PreviewRegistry`, `PreviewCache`, `Previewer` have zero test coverage | Preview correctness unverified | **High — Phase 11** |
| `Command::NavigateForward` is absent from the enum; only `NavigateBack` exists | Forward navigation broken | **Medium** |
| `test_local_fs_list_not_found` and `test_local_fs_read_not_found` fail (2 known failures) | VFS error path correctness | **Medium** |
| `actors/cache.rs` `Cache` struct is `todo!()` but imported and compiled | Dead code in production binary | **Low — Phase 12** |
| Preview providers call `tokio::fs` directly for magic-byte detection (registry.rs `read_header`) | Violates VFS abstraction for remote preview | **Low — Phase 11** |

---

## Feature Flag Reference

| Flag | Enables | Additional deps |
|------|---------|----------------|
| `metadata-image` | Image dimensions + full EXIF | `imagesize`, `kamadak-exif` |
| `metadata-audio` | ID3 tags + duration | `id3` |
| `metadata-video` | Video dimensions, duration, codecs | `mp4parse` |
| `metadata-document` | PDF page count, title, author | `lopdf` |
| `metadata-archive` | Archive entry listing (ZIP/TAR/7Z/…) | `zip`, `tar`, `sevenz-rust2`, decompressors |
| `metadata-archive-rar` | Above + RAR | `unrar` (C++, RARlab license) |
| `metadata` | All extractors except RAR | — |
| `preview-code` | Syntax-highlighted code preview | `syntect` |
| `preview-image` | Image thumbnail generation | `image` |
| `crypto` | AES-GCM / XChaCha20 + Argon2id vault | `aes-gcm`, `chacha20poly1305`, `argon2` |
| `s3` | AWS S3 backend | `aws-sdk-s3` |
| `webdav` | WebDAV backend | `reqwest` |
| `sftp` | SSH/SFTP backend | `ssh2` |
| `fuse` | FUSE mount of any `FsProvider` | `fuser` |
| `k8s` | Kubernetes resource browser | `kube` |
| `all-features` | Everything including RAR | — |

> **Compile-time rule:** `cargo build -p filer-core` (no features) must complete in under 30 seconds on a mid-range developer machine. Feature additions that push base build time above this threshold require explicit justification.
