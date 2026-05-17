# Filer

A fast, modern file explorer built in Rust.

Current milestone: `0.2.3`.

## Architecture

```
filer/
├── filer-core/         # Core library (actors, VFS, search, preview)
├── filer-app/          # Iced-based GUI application
└── filer-ecosystem/    # Extension contracts, packages, profile sync
```

## Design

- **Actor-based**: Independent workers communicate via message passing
- **Async-first**: Non-blocking operations, streaming results
- **Abstracted VFS**: Support for local files, archives, and more
- **Extension-aware**: Extensions can enrich core results with semantic data
  such as git status, file decorations, metadata, commands, previews, and
  provider capabilities.

## Extension Direction

Filer's extension system is meant to improve the file-manager mechanics, not
replace the core product. `filer-core` remains responsible for dependable
navigation, scanning, search, file operations, provider access, sessions,
pipeline state, and event routing.

Extensions run through core-controlled contracts and produce structured,
semantic output. For example, a git extension should report that a file is
modified or added; the desktop app or future web client decides how to render
that state as a badge, color token, tooltip, or row decoration. This keeps the
same extension useful across clients without letting extensions depend on one
UI framework.

The near-term priority is core contract stabilization, not a full plugin
platform or app rewrite. Request identity and stale-event guards are now in
place for scan, search, preview, refresh, and their direct-path `Location`
entrypoints, and file operations now emit correlated operation progress,
completion, and error events. App-facing errors now carry `ErrorKind`, stable
`ErrorCode`, optional `ErrorTarget`, and a recoverability flag derived from the
code. Core also emits structured `tracing` events when `CoreError` becomes
`Event::Error`, leaving subscriber setup to the application. The project should
next build on the new `Location` foundation by migrating provider addressing
where it matters, then settle large-directory paging/streaming contracts,
extension output envelopes, and the boundary between app-local config and
future profile sync.

`0.2.3` hardens the additive `Location` contract for provider-aware addressing.
`LocationRef` now has explicit id-only, descriptor-only, and full modes instead
of optional fields, and `LocationDescriptor` separates the provider root from
ordered `LocationSegment` layers. `LocationId` hashes the root plus ordered
segments, but not display text. `LocationRoute` now classifies descriptors as
direct local paths, segmented locations, or unsupported provider routes, with a
derived route cache in the registry. Direct local `Location` commands are wired
through API routing for navigation, scan, search, preview, metadata, and
extended metadata, and the actor tests now check cancellation, stale-result
suppression, cache reuse, and refresh behavior for those paths. Local file
listing now has explicit detail options: scans default to fast rows that avoid
per-entry stat calls, while metadata listings can be requested when size,
timestamps, or permissions are needed. This does not yet replace public
command/event `PathBuf` and `NodeId` surfaces; it strengthens the compatibility
layer for that migration. The intent is for `Location` to become the bridge
across local files, remote providers, virtual providers, extension-backed
providers, and archives. Nested archive addresses can now be modeled as a
provider root plus archive/member segments instead of forcing every layer into
one path string. Large-directory paging/streaming, archive navigation, and
wire-safe transport envelopes remain future work.

Extensions should be introduced through one complete vertical slice before the
platform grows wider. The reference slice is git file decorations: core exposes
visible files to an extension, the extension emits semantic states such as
modified or added, and each client renders those states in its own UI.

The first extension host may be a trusted in-process prototype. That is
deliberately not a marketplace-safe plugin system. Sandboxing, WASM hosting,
package installation, and extension manager UI come later if the trusted slice
proves useful without adding too much overhead.

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run -p filer-app
```

## License

MIT
