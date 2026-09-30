# Filer

Filer is a file explorer written in Rust for people who browse large folders and source trees. Opening a folder with tens of thousands of entries should not freeze the window, and Git status should arrive without delaying the listing. Filer keeps all file-system behavior in one engine, `filer-core`, so every client gets the same paging, cancellation, and error handling.

## How it works

A client sends commands such as `Navigate` or `Search` to `filer-core` and receives results as events. Each request carries an id, and core discards results that a newer request in the same session has superseded. Directory listings arrive in pages. On a 10,000-file directory, the first 256-row page takes about 0.3 ms through the public API on the [recorded baseline machine](crates/filer-core/benches/baselines/2026-09-05-core-021-linux-i7-11800h-btrfs.md).

Files are addressed by `Location` rather than by path. A Location names a provider and an ordered list of nested layers, so the same model addresses a local file, a member inside a ZIP archive, or a file behind another provider.

Extensions report meaning, not pixels. A Git extension reports that a file is modified, and each client decides whether to show that as a badge, a color, or a tooltip. The same extension then works in any client without depending on a UI framework.

## Crates

| Crate | Purpose |
|---|---|
| [`filer-core`](crates/filer-core/README.md) | File-manager engine: sessions, navigation, scan, search, preview, watch, file operations, and providers |
| [`filer-app`](crates/filer-app/README.md) | Desktop client built on Iced |
| [`filer-ecosystem`](crates/filer-ecosystem/README.md) | Serializable contracts for extensions, packages, and profile sync |
| [`taskroot`](tools/taskroot/README.md) | Markdown task tracker used to plan this repository |
| `filer-task-web` | Localhost web board for `taskroot` projects |

## Build and run

```bash
cargo build --release --workspace --exclude filer-app
cargo test -p filer-core
cargo run -p filer-core --example navigate -- <directory>
```

The desktop app does not compile against the current core API, so these commands exclude it. The [filer-app README](crates/filer-app/README.md) explains what replaces it.

## Learn more

- [ROADMAP.md](ROADMAP.md) explains where the project is heading.
- [CHANGELOG.md](CHANGELOG.md) records what each milestone changed.
- [CONTEXT.md](CONTEXT.md) defines the domain terms used across the code and docs.
- [docs/](docs/README.md) holds architecture, contracts, and benchmark design.

## License

MIT
