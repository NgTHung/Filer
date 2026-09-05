# filer-app

`filer-app` is the Iced desktop frontend for Filer. It is the user-facing file
manager that consumes `filer-core` for navigation, search, preview generation,
file operations, and filesystem events.

Active work: app:UI-011, a minimal validation client alongside core 0.3.1.
It covers one window, Location-native browsing, paging, and asynchronous Git
decorations with a provisional renderer. See the
[validation scope](../docs/architecture/filer-app.md#active-validation-track).
Implementation is tracked in UI-012 through UI-015. Full framework evaluation
and the app rewrite remain deferred.

The app target is simple: make local file management feel clean, fast, and
predictable before expanding into more advanced workflows. The visual direction
is inspired by Windows Explorer and Files Community: quiet surfaces, clear
hierarchy, compact controls, and a polished details list. The workflow intent is
closer to Xplorer: efficient navigation, quick access, useful context actions,
and fast feedback while working with files.

## Current Capabilities

The list below describes the legacy app. Its source still uses retired core
identities; it is not evidence of compatibility with the current core. The
validation track creates an isolated entry point and records its launch command
when that target exists.

- Local folder navigation through the core navigation module.
- Details-list file view with sortable columns.
- Quick Access places, bookmarks, and recent folders.
- Search with debounced input and streamed results.
- Right-side Preview and Details panel.
- Basic file operations: copy, cut, paste, rename, create folder, and delete.
- Context-menu actions for selected files and folders.
- Bottom status bar with item, selection, search, and sort state.
- Light, dark, and automatic theme modes.

## Extension Rendering Direction

The app should render extension output that comes through core as structured
semantic data. A git extension, for example, should not directly style Iced
widgets. It should report states such as modified, added, untracked, ignored,
or conflicted for visible files. The app can then render those states as badges,
filename color tokens, tooltips, row decorations, or panel/status content using
the active theme and layout.

This keeps the desktop app aligned with future web clients: core and extensions
agree on meaning, while each client owns presentation.

The app may need a substantial refactor, but that should follow core contract
stabilization. Bugs that reveal contract problems, such as stale search results,
duplicate directory loads, preview races, or large-directory limits, should feed
back into core first. Pure UI issues, such as context-menu placement or visual
polish, can wait for the app refactor.

For `0.2.0`, the app consumes the new core request IDs, operation IDs, and
structured error categories. The broader app refactor remains deferred until the
remaining core contracts are clearer.

The next useful visible proof is not a full app rewrite. It is a large folder
that appears quickly and remains responsive, followed by asynchronous semantic
decorations such as git badges. The app should treat extension output as a late,
optional enhancement over already usable directory data.

## Design Goals

- Look good enough to use daily while the larger feature roadmap is still in
  progress.
- Keep the first screen as the actual file manager, not a landing page or demo
  shell.
- Prefer a Files-style interface: simple, elegant, readable, and native-feeling.
- Keep Xplorer-style intent: dense enough for work, fast to scan, and focused on
  file-management actions.
- Make right-click, preview, search, the topbar, the status bar, and Quick Access
  feel complete before adding larger features.

## Running

```bash
cargo run -p filer-app
```

## Building

```bash
cargo build -p filer-app
cargo build -p filer-app --release
```

## Testing

```bash
cargo test -p filer-app
cargo check --workspace
```

For core behavior, run:

```bash
cargo test -p filer-core --lib
```

## Known Limitations

- The app is currently local-first; remote providers in `filer-core` are not yet
  surfaced as first-class UI locations.
- The primary file view is details-list only. Tile/grid views are planned later.
- Drag and drop is not complete yet.
- Preview quality depends on the current core preview providers and available
  feature flags.
- Advanced workflows such as tabs, split panes, command palette, and operations
  history are planned but not part of the initial polish pass.
- Extension UI is not a plugin-rendered widget system yet. The intended model is
  client-rendered semantic output from core-hosted extensions.
