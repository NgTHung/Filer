# filer-core

Core library for the Filer file explorer.

## Modules

| Module | Purpose |
|--------|---------|
| `api/` | Public interface (commands, events, handle) |
| `model/` | Data structures (FileNode, FileTree, Query) |
| `actors/` | Concurrent workers (scanner, searcher, watcher, previewer) |
| `bus/` | Message routing between actors |
| `vfs/` | Virtual filesystem abstraction |
| `pipeline/` | Data transformations (filter, sort, group) |
| `services/` | Feature modules (mime, metadata, preview) |
| `utils/` | Shared helpers (path, size, time) |

## Extension Boundary

`filer-core` owns the file-manager kernel: sessions, navigation, scanning,
search dispatch, watching, file operations, provider access, pipeline
execution, cache invalidation, cancellation, and event routing.

Extensions should enhance that kernel rather than replace it. They can add
commands, providers, previews, metadata, file decorations, status badges, and
other semantic outputs through core-controlled contracts. Clients such as the
desktop app or future web UI should render those outputs in their own native
interface.

For example, a git extension should publish semantic file states like
`modified`, `added`, or `untracked`. Core routes those states to clients; the
client decides whether to show an `M` badge, a color token, a tooltip, or a side
panel entry.

## Current Core Priority

The next core milestone is contract stabilization. Before a major app rewrite or
full extension runtime, core should settle:

- request ids and stale-event guards for scan, search, preview, and refresh
- operation ids for progress and completion events
- structured recoverable errors
- provider addressing beyond raw local paths
- cancellation and timeout semantics
- large-directory loading behavior
- extension output envelopes and file decoration payloads
- the boundary between app-local config, core session snapshots, provider
  profiles, extension profile state, and future sync

Built-in modules should become extension-aware where useful, but navigation,
scan, search orchestration, watch, file operations, sessions, provider routing,
cache, pipeline, and event delivery remain core kernel behavior.

The provider-addressing contract should be structured, not just a custom string
parser. A future `Location` should be able to represent a scheme, provider or
profile id, internal path, optional archive/member path, display path, and
capabilities. This keeps local, archive, remote, virtual, and extension-backed
files addressable through the same core model.

Core stabilization is complete only when large directory loading is bounded,
stale scan/search/preview events are rejected, operation progress is correlated,
recoverable errors are structured, archive traversal is modeled as provider
navigation, and the trusted git-decoration prototype proves extension output can
arrive after directory data without blocking it.

## Usage

```rust
use filer_core::{FilerCore, Command, Event};

#[tokio::main]
async fn main() {
    let core = FilerCore::new().await.unwrap();
    
    // Send command
    core.send(Command::Navigate("/home".into())).unwrap();
    
    // Receive events
    while let Ok(event) = core.event_receiver().recv() {
        match event {
            Event::DirectoryLoaded { groups, .. } => {
                println!("Loaded {} files", groups.total_count);
            }
            _ => {}
        }
    }
}
```

## Dependencies

- `tokio` — Async runtime
- `flume` — Channels for actor communication
- `async-trait` — Async trait support
