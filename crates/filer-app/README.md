# filer-app

`filer-app` is the Filer desktop client. It exists to make local file management feel fast and predictable: a large folder should appear at once and stay responsive while extra detail, such as Git status, fills in afterward. The app owns presentation and interaction. `filer-core` owns navigation, directory data, search, previews, and file operations, so the app never touches the file system directly.

## Build status

The app does not compile against the current `filer-core` API. Its source uses path and `NodeId` addressing that core does not provide. Build the rest of the workspace with:

```bash
cargo build --workspace --exclude filer-app
```

A minimal validation client that uses the current `Location` contracts replaces it first. Its scope is one window, folder browsing with paging, and asynchronous Git decorations. The [app architecture](../../docs/architecture/filer-app.md#active-validation-track) defines that client, and `.tasks/app` tracks its progress.

## Design direction

The visual direction follows Windows Explorer and Files Community: quiet surfaces, clear hierarchy, compact controls, and a readable details list. The workflow follows Xplorer: quick navigation, useful context actions, and immediate feedback. The first screen is the file manager itself, not a landing page.

The app renders extension output, but extensions never draw widgets. A Git extension reports that a file is modified, added, untracked, ignored, or conflicted. The app turns that state into a badge, a filename color, a tooltip, or a row decoration from the active theme. A web client can render the same state its own way.

Decorations are late and optional. The app shows directory rows first and applies decorations when they arrive, so a slow `git status` never delays a listing.

## Where to report problems

A bug that shows a contract problem, such as stale search results, duplicate directory loads, preview races, or slow large folders, belongs in core. File it as a core task. A bug in layout or visual polish, such as context-menu placement, belongs to the app.

## More detail

- [App architecture](../../docs/architecture/filer-app.md) defines ownership boundaries, state, and the framework adapter contract.
- [ROADMAP.md](ROADMAP.md) lists the product features the app should reach.
- [Core API reference](../../docs/core-api.md) describes the commands and events the app consumes.
