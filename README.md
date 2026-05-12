# Filer

A fast, modern file explorer built in Rust.

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
place for scan, search, preview, and refresh flows. The project should next
settle operation identity, structured errors, provider addressing,
large-directory loading contracts, extension output envelopes, and the boundary
between app-local config and future profile sync.

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
