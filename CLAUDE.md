# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Conventions

This project follows **TDD (Test-Driven Development)**. Tests are written before implementation for each item in `ROADMAP.md`. When working on any unfinished phase, write the test file first, then implement to make it pass.

Integration tests live in `filer-core/src/tests/infra/` — one file per domain (`mime_test.rs`, `metadata_test.rs`, `table_test.rs`, etc.). Do **not** use inline `#[cfg(test)]` modules inside source files; put tests in the corresponding infra file instead.

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

# Full metadata test suite (required when touching services/metadata or services/mime)
cargo test -p filer-core --features "metadata"

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

Key methods:
- `read(path)` → `Vec<u8>` — full file contents
- `read_range(path, start, len)` → `Vec<u8>` — partial read
- `read_header(path, n_bytes)` → `Vec<u8>` — first N bytes for MIME detection
- `open_reader(path)` → `Box<dyn ReadSeek>` — synchronous `Read + BufRead + Seek` handle

`ReadSeek` is a blanket object trait (`Read + BufRead + Seek + Send`). `LocalFs` returns a real `BufReader<std::fs::File>` (zero extra allocation). Remote providers fall back to `Cursor<Vec<u8>>` via the default impl. Extraction crates (`zip`, `kamadak-exif`, `id3`, `mp4parse`, `lopdf`) all accept `impl Read` or `impl Read + Seek`; pass the reader from `open_reader` directly.

**Rule**: extractors must never call `std::fs::File::open` or `tokio::fs` directly — always go through the provider so remote backends work transparently.

### Services Layer (`src/services/`)

#### MIME Service (`services/mime/`)

Three-tier detection:
1. **Extension lookup** — `table::lookup_extension(ext)` searches `EXT_TABLE` (binary search, sorted). This is the authoritative source for ~110 common extensions and takes priority over everything else for non-ambiguous extensions.
2. **Magic bytes** — `infer::get(bytes)` via `detect_from_bytes()` covers ~70 formats.
3. **Fallback** — `new_mime_guess::from_ext(ext)` is last resort only. It uses the IANA registry and maps many code-file extensions (`.py`, `.rs`, `.go`) to `text/plain` — do not rely on it for those.

`MimeDetector::detect_from_path()` consults `EXT_TABLE` first (for non-ambiguous extensions), then falls back to `new_mime_guess`. `detect()` returns early when extension confidence is `Definitive`, preventing ZIP magic bytes from overriding `.docx` → Document.

Ambiguous extensions (`bin`, `dat`, `raw`, `out`, `tmp`, `txt`, `log`) are handled separately and never looked up in `EXT_TABLE`.

#### Metadata Service (`services/metadata/`)

`MetadataExtractor` trait with `extract(path, mime, provider)`. `MetadataRegistry` routes by `MimeCategory` to the first matching extractor.

Feature-gating convention in every extractor body:
```rust
async fn extract(&self, path, mime, provider) -> Result<ExtendedMetadata, CoreError> {
    #[cfg(not(feature = "metadata-image"))]
    return Ok(ExtendedMetadata::Unavailable);

    #[cfg(feature = "metadata-image")]
    { /* real implementation using provider.open_reader() */ }
}
```

Helper methods that use optional crates are also gated with `#[cfg(feature = "...")]`.

#### Preview Service (`services/preview/`)

Not yet fully implemented (Phase 8 in ROADMAP). Stubs exist for `PreviewProvider` trait, `PreviewRegistry`, and `PreviewCache`.

### Pipeline System (`src/pipeline/`)

Composable transformation stages (filter, sort, group) applied to `Vec<FileNode>`. Frontend sends a serializable `PipelineConfig`; core builds and runs the `Pipeline`. Output is always `GroupedNodes` (even if ungrouped, it's a single group).

### Sessions (`src/model/`)

`SessionManager` issues `SessionId`s. Each session has its own event channel. All `Event` variants carry a `SessionId` for routing in multi-session scenarios.

### Error Handling (`src/errors.rs`)

Custom `CoreError` enum. Use `CoreError::from_io_error(err, path)` for converting `std::io::Error` with path context — **error first, path second**. The `From<IoError>` impl provides a fallback without path.

## Key Files

- `filer-core/src/api/handle.rs` — `FilerCore` public entry point
- `filer-core/src/api/module.rs` — `Module` trait and `HandlerRegistry`
- `filer-core/src/api/commands.rs` — `Command` enum
- `filer-core/src/api/events.rs` — `Event` enum
- `filer-core/src/modules/` — Six built-in modules, each with its own actor
- `filer-core/src/vfs/provider.rs` — `FsProvider` trait and `ReadSeek` object trait
- `filer-core/src/vfs/local.rs` — `LocalFs` implementation
- `filer-core/src/pipeline/mod.rs` — Pipeline execution
- `filer-core/src/errors.rs` — `CoreError` and conversion helpers
- `filer-core/src/services/mime/detector.rs` — `MimeDetector`, detection logic
- `filer-core/src/services/mime/table.rs` — `EXT_TABLE`, authoritative extension lookup
- `filer-core/src/services/metadata/extractor.rs` — `MetadataExtractor` trait and `MetadataRegistry`
- `filer-core/src/services/metadata/extended.rs` — `ExtendedMetadata` and all metadata structs
- `filer-core/src/services/metadata/extractors/` — Six feature-gated extractor implementations
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
| `metadata-image` | Image dimensions + EXIF (`imagesize`, `kamadak-exif`) |
| `metadata-audio` | Audio tags and stream info (`id3`) |
| `metadata-video` | Video dimensions and duration (`mp4parse`) |
| `metadata-document` | PDF page count and metadata (`lopdf`) |
| `metadata-archive` | Archive entry listing (`zip`, `tar`, `flate2`, `bzip2`, `xz2`, `zstd`, `sevenz-rust2`) |
| `metadata-archive-rar` | Above + RAR support (`unrar` — C++ dependency, RARlab license) |
| `metadata` | All of the above except RAR |
| `all-features` | Everything including `crypto`, all remote backends, and `metadata-archive-rar` |

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **Filer** (2839 symbols, 6864 relationships, 140 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/Filer/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/Filer/context` | Codebase overview, check index freshness |
| `gitnexus://repo/Filer/clusters` | All functional areas |
| `gitnexus://repo/Filer/processes` | All execution flows |
| `gitnexus://repo/Filer/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
