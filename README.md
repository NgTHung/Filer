# Filer

A fast, modern file explorer built in Rust.

## Architecture

```
filer/
├── filer-core/    # Core library (actors, VFS, search, preview)
├── filer-gui/     # Iced-based GUI application
└── filer-app/     # Binary entry point
```

## Design

- **Actor-based**: Independent workers communicate via message passing
- **Async-first**: Non-blocking operations, streaming results
- **Abstracted VFS**: Support for local files, archives, and more
- **Extensible**: Plugin-ready preview and metadata systems

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
