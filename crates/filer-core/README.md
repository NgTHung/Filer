# filer-core

`filer-core` is the file-manager engine that every Filer client shares. A client sends commands and renders the events that come back. Core does the file-system work: listing, searching, previewing, watching, and changing files. A desktop app, a web client, and an extension all see the same paging, cancellation, cache, and error behavior, and none of them reimplements it.

## What core owns

Core owns sessions and command routing, navigation, directory scanning, search, file watching, previews, metadata, and file operations. It reaches files through provider contracts. It applies filters, sorting, and grouping in one pipeline. It correlates every result with the request that caused it, drops stale results, and invalidates caches after writes.

Core does not render anything and depends on no UI framework. Extensions add providers, previews, metadata, and semantic output such as Git status through core-owned contracts, and each client decides how to show that output. The full rule set lives in [architecture invariants](../../docs/architecture/invariants.md).

## How a request flows

1. A client calls `Command::Handshake` and receives a session id.
2. It sends a command such as `Navigate` with that session and a fresh `RequestId`.
3. A module actor does the work off the client's thread and emits events that echo the request id.
4. If the client sends a newer request in the same session, core drops results from the older one.

Directory listings arrive in pages of 256 rows by default. Streaming pages stop reading the directory once the page is full, so the first page of a 10,000-entry folder arrives in about 0.3 ms on the [recorded baseline machine](benches/baselines/2026-09-05-core-021-linux-i7-11800h-btrfs.md). Sorting needs every row first, so a sorted first page costs a full walk.

Files are addressed by `Location`, not by path. A Location records a provider, a root, and ordered nested segments, so one model covers a local file and a member inside a ZIP archive. Commands take a `LocationRef`, which carries a compact id, the full descriptor, or both.

Errors arrive as `Event::Error` with a stable `ErrorCode` for branching, a display message, and the request or operation that failed.

## Usage

This example lists the first page of a directory. It lives in [`examples/navigate.rs`](examples/navigate.rs), so `cargo test` keeps it compiling.

```rust
use filer_core::{Command, Event, FilerCore, Location, LocationRef, RequestId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    let core = FilerCore::with_defaults();
    let events = core.event_receiver();

    core.send(Command::Handshake)?;
    let session = loop {
        if let Event::SessionCreated(session) = events.recv_async().await? {
            break session;
        }
    };

    let request = RequestId::new();
    core.send(Command::Navigate {
        location: LocationRef::from_location(&Location::local(path)),
        session,
        request,
    })?;

    while let Ok(event) = events.recv_async().await {
        match event {
            Event::DirectoryPageLoaded {
                groups,
                request: loaded,
                ..
            } if loaded == request => {
                println!("first page has {} rows", groups.total_count);
                break;
            }
            Event::Error {
                message,
                request: Some(failed),
                ..
            } if failed == request => return Err(message.into()),
            _ => {}
        }
    }
    Ok(())
}
```

Run it with `cargo run -p filer-core --example navigate -- <directory>`.

`FilerCore::with_defaults()` loads every built-in module over the local filesystem with a 128 MB directory cache. `FilerCore::new()` starts only the router, so you load modules yourself.

## Source layout

| Module | Purpose |
|---|---|
| `api/` | Public commands, events, wire format, and the `FilerCore` handle |
| `model/` | Shared data types such as `Location`, ids, and page state |
| `actors/` | Actor runtime and command router |
| `modules/` | Navigation, scan, search, watch, preview, operation, and extension workers |
| `vfs/` | Provider contracts, local filesystem, watching, and archive routing |
| `pipeline/` | Filter, sort, group, and paging policy |
| `services/` | MIME detection, metadata, preview, and cache services |
| `utils/` | Shared helpers |

## Testing

```bash
cargo test -p filer-core
python3 crates/filer-core/tests/check_features.py
```

The second command checks every feature combination. See [tests/README.md](tests/README.md) for its options and [benches/README.md](benches/README.md) for performance measurement.

## More detail

- [Core API reference](../../docs/core-api.md) covers commands, paging and cursors, cache invalidation, progress, and errors.
- [CHANGELOG.md](../../CHANGELOG.md) records removed contracts and how to migrate from them.
- [CONTEXT.md](../../CONTEXT.md) defines the domain terms.
