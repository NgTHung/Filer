# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build
cargo build -p filer-core
cargo build --release

# Test all
cargo test

# Test specific crate
cargo test -p filer-core

# Test specific module/function
cargo test -p filer-core navigation_flow
cargo test -p filer-core -- --nocapture

# With optional features
cargo build -p filer-core --features "crypto"
cargo test -p filer-core --features "crypto"

# Lint
cargo clippy -p filer-core
cargo fmt --check
```

## Architecture

This is a **modular file manager core** (`filer-core`) with a placeholder binary (`filer-app`). The design separates all filesystem logic into `filer-core` so it can be composed with any frontend.

### Command → Event Flow

The public API (`FilerCore` in `src/api/handle.rs`) accepts `Command` variants and emits `Event` variants. Internally:

1. `FilerCore::send(command)` → `CommandRouter` actor
2. `CommandRouter` → looks up handler in `HandlerRegistry` by `Command::key()` (string)
3. Handler (registered by a module) → sends message to module's actor
4. Actor → emits `Event` back via session event channel

### Module System (`src/api/module.rs`)

Modules are loaded via `FilerCore::load(module)`. Each module implements `Module::init(ctx)` to:
- Register command handlers in `HandlerRegistry`
- Spawn actors into `ActorSystem`

Built-in modules: `ScanModule`, `NavigationModule`, `WatchModule`, `PreviewModule`, `OperationsModule`, `SearchModule`.

### Actor Infrastructure (`src/actors/`)

Actors implement `Actor { run() → Future, name() }` and are spawned/tracked by `ActorSystem`. Communication is via `flume` channels. Shutdown cascades naturally: aborting the top-level actor closes its sender, which causes downstream actors to exit when their receiver detects closure.

### VFS Abstraction (`src/vfs/`)

`FsProvider` trait abstracts filesystem operations. `LocalFs` is the only complete implementation. Feature-gated stubs exist for S3, WebDAV, SFTP, FUSE, K8s, and ZIP archives.

### Pipeline System (`src/pipeline/`)

Composable transformation stages (filter, sort, group) applied to `Vec<FileNode>`. Frontend sends a serializable `PipelineConfig`; core builds and runs the `Pipeline`. Output is always `GroupedNodes` (even if ungrouped, it's a single group).

### Sessions (`src/model/`)

`SessionManager` issues `SessionId`s. Each session has its own event channel. All `Event` variants carry a `SessionId` for routing in multi-session scenarios.

### Error Handling (`src/errors.rs`)

Custom `CoreError` enum. Use `CoreError::from_io_error(path, err)` for converting `std::io::Error` with path context. The `From<IoError>` impl provides a fallback without path.

## Key Files

- `filer-core/src/api/handle.rs` — `FilerCore` public entry point
- `filer-core/src/api/module.rs` — `Module` trait and `HandlerRegistry`
- `filer-core/src/api/commands.rs` — `Command` enum
- `filer-core/src/api/events.rs` — `Event` enum
- `filer-core/src/modules/` — Six built-in modules, each with its own actor
- `filer-core/src/vfs/provider.rs` — `FsProvider` trait
- `filer-core/src/pipeline/mod.rs` — Pipeline execution
- `filer-core/src/errors.rs` — `CoreError` and conversion helpers
- `filer-core/src/tests/` — Integration test suite

## Optional Features

| Feature | Enables |
|---------|---------|
| `crypto` | AES-GCM/ChaCha20 encryption, Argon2/scrypt key derivation |
| `s3` | AWS S3 backend |
| `webdav` | WebDAV backend |
| `sftp` | SSH/SFTP backend |
| `fuse` | FUSE filesystem mount |
| `k8s` | Kubernetes backend |
