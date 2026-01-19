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
            Event::DirectoryLoaded { entries, .. } => {
                println!("Loaded {} files", entries.len());
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
