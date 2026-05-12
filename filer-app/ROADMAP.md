# filer-app Final Feature Checklist

This roadmap tracks the Iced desktop app. The root `ROADMAP.md` remains the
engineering roadmap for `filer-core` and broader crate milestones; this document
tracks the final product shape of the user-facing app.

## Product Target

Filer should be a fast Explorer replacement with programmer support. It should
stay a file manager first: clean navigation, fast browsing, dependable file
operations, rich previews, and developer tools that help with real work without
turning the app into a bloated IDE.

Extensions should improve the file-manager experience by sending semantic data
through core. The app should render those semantics as native Iced UI: row
badges, status badges, color tokens, tooltips, context actions, panels, previews,
and settings. Extensions should not own desktop widgets directly.

The app roadmap is intentionally downstream of core contract stabilization. A
large app refactor should wait until structured errors, provider addressing,
extension output envelopes, and large-directory loading contracts are settled.
Request ids, stale-event guards, and operation ids are already available in core
for app-facing async work. App bugs that expose remaining
contracts should be treated as core feedback; pure visual and interaction bugs
can be fixed during the app refactor.

The next visible proof should be narrow: a large folder renders without blocking
and extension decorations can appear afterward. The app must not wait for
decoration output before showing primary directory data unless a future feature
explicitly marks itself as blocking.

Milestone labels:

- `MVP`: required for a reliable local Explorer replacement.
- `Polish`: quality, persistence, and interaction refinement.
- `Power`: programmer-oriented and advanced file-manager workflows.
- `Ecosystem`: extensions, themes, and third-party capability surfaces.
- `Future`: useful later work that should not block the main product.

## Current Baseline

- [x] `MVP` Local navigation through `filer-core`.
- [x] `MVP` Back, forward, up, refresh, and manual address bar navigation.
- [x] `MVP` Quick Access places, user bookmarks, recent folders, and Windows
  drive entries.
- [x] `MVP` Details-list file view with sortable columns.
- [x] `MVP` Group controls for none, extension/type, date, size, and first
  letter.
- [x] `MVP` Debounced search with streamed results.
- [x] `MVP` Right-click context menu with pointer-based positioning.
- [x] `MVP` Copy, cut, paste, delete, rename, create folder, and create file.
- [x] `MVP` Preview/Details side panel backed by core preview providers.
- [x] `MVP` Bottom status bar with item, selection, search, sort, and preview
  state.
- [x] `MVP` Active-directory watcher hook with debounced refresh.
- [x] `MVP` Fresh refresh path that invalidates stale directory cache entries.
- [x] `MVP` Core request ids for scan, search, preview, and refresh commands.
- [x] `MVP` Core operation ids for operation progress/completion correlation.
- [x] `MVP` Light, dark, and automatic theme modes.
- [x] `MVP` Broad UI-core tracing for commands, events, watcher refresh, and
  preview flow.

## Navigation And Workspace

- [x] `MVP` Navigate by sidebar places and recent folders.
- [x] `MVP` Navigate by typed path in the address bar.
- [ ] `MVP` Clickable breadcrumb segments.
- [ ] `MVP` Restore previous folder on app start.
- [ ] `Polish` Persist recent folders and bookmarks through simple app-local
  config, not the future ecosystem sync operation log.
- [ ] `Polish` Persist sort, group, preview panel, and layout preferences in
  app-local config until core session snapshots are stable.
- [ ] `Power` Tabs with independent sessions.
- [ ] `Power` Split panes with independent sessions.
- [ ] `Power` Persistent workspaces that restore tabs, split panes, selected
  folder, and panel state.
- [ ] `Power` Project/workspace detection for code folders.
- [ ] `Future` Session sync across desktop/web frontends.

## File Views

- [x] `MVP` Details view with name, type, size, and modified columns.
- [x] `MVP` Sort by name, type, size, and modified date.
- [x] `MVP` Group by extension/type, date, size, and first letter.
- [ ] `MVP` Hidden-file toggle.
- [ ] `MVP` Window-aware context menu bounds.
- [ ] `Polish` Compact/list density modes.
- [ ] `Polish` Per-folder view preferences.
- [ ] `Polish` Empty-folder and empty-search states with useful actions.
- [ ] `Power` Grid/icon view.
- [ ] `Power` Gallery/media view.
- [ ] `Power` Tree view/sidebar for nested project browsing.
- [ ] `Power` Large-directory virtualization.
- [ ] `Core` Align file-list rendering with the chosen core directory loading
  contract: full snapshot, page, delta, or incremental event stream.

## File Operations

- [x] `MVP` Copy, cut, paste, delete, rename, create folder, and create file.
- [x] `MVP` Operation completion refreshes the current directory.
- [ ] `MVP` Consistent modal pattern for rename, create folder, and create file.
- [ ] `MVP` Clear duplicate-name and permission-denied feedback.
- [ ] `MVP` Multi-selection-aware context actions.
- [ ] `Polish` Drag and drop for internal move/copy.
- [x] `Polish` Operation progress tray for long copy/move/delete operations.
- [ ] `Polish` Conflict resolution for copy/move collisions.
- [ ] `Core` Render future undo/conflict metadata once the core operation
  contract exists; full undo UI can wait.
- [ ] `Power` Operation queue and operation history.
- [ ] `Power` Undo for reversible operations where practical.
- [ ] `Power` Bulk rename.
- [ ] `Power` Archive create, extract, compress, and decompress actions.
- [ ] `Future` File sync and backup workflows.

## Search And Command Palette

- [x] `MVP` Debounced folder search.
- [x] `MVP` Streamed search result count and status state.
- [ ] `MVP` Search clear/focus behavior that feels native.
- [ ] `Polish` Advanced search syntax UI hints for core query filters.
- [ ] `Polish` Search within current folder, project, or selected roots.
- [ ] `Power` Global command palette.
- [ ] `Power` Recent commands and frequently used actions.
- [ ] `Power` Palette commands for files, git, terminal, providers, converters,
  settings, and extensions.
- [ ] `Power` Keyboard shortcut discoverability.
- [ ] `Ecosystem` Render extension-contributed commands only from validated
  core/app registry state.

## Previewer And Metadata

- [x] `MVP` Preview/Details panel shell.
- [x] `MVP` Core preview provider integration.
- [x] `MVP` Image thumbnail bytes stored for loaded previews.
- [x] `MVP` Stale preview guards when selection changes quickly.
- [ ] `MVP` Clear unsupported, loading, error, folder, binary, and empty states.
- [ ] `Polish` Text and code preview with monospace layout and truncation.
- [ ] `Polish` Syntax-highlighted code preview when `preview-code` is enabled.
- [ ] `Polish` Image viewer with zoom, fit, actual size, and transparent
  background handling.
- [ ] `Power` Video player preview.
- [ ] `Power` Audio player preview.
- [ ] `Power` Markdown/document preview.
- [ ] `Power` Archive preview with entry browsing.
- [ ] `Power` Hex/binary preview.
- [ ] `Power` Rich metadata/details table from `filer-core`.
- [ ] `Power` Thumbnail disk cache keyed by stable file identity or content
  hash.
- [ ] `Ecosystem` Pluggable preview providers.
- [ ] `Ecosystem` Render extension preview/metadata payloads from
  client-neutral core events.

## Programmer Features

- [ ] `MVP` Open selected folder in external terminal.
- [ ] `MVP` Open selected file/folder in configured editor.
- [ ] `Future` Integrated terminal panel scoped to the current folder only if it
  stays an external helper, not an IDE surface.
- [ ] `Ecosystem` Render git file decorations as the first extension vertical
  slice.
- [ ] `Future` Git panel with status, diff, branch, commit, stash, pull, and
  push actions only if it stays a lightweight file-manager helper.
- [ ] `Power` SSH/SFTP browsing as a first-class location.
- [ ] `Power` Project-aware Quick Access entries.
- [ ] `Power` File converters for common developer assets and documents.
- [ ] `Power` Syntax-aware file metadata where it helps browsing.
- [ ] `Power` Task/script launcher for project-local commands.
- [ ] `Future` Lightweight project diagnostics from external tools.
- [ ] `Ecosystem` Render file decorations from extensions, including git-style
  modified, added, deleted, untracked, ignored, conflicted, and clean states.

## Providers And Remote Filesystems

- [x] `MVP` Local filesystem provider surfaced in the app.
- [ ] `Power` Archives as navigable folders.
- [ ] `Power` SFTP/SSH provider UI.
- [ ] `Power` WebDAV provider UI.
- [ ] `Power` S3 provider UI.
- [ ] `Power` Vault/encrypted provider UI.
- [ ] `Power` Provider connection manager.
- [ ] `Power` Provider capability badges for read, write, watch, and search.
- [ ] `Future` FUSE mount management.
- [ ] `Future` Kubernetes resource browser UI.

## Extensions And Automation

- [x] `Ecosystem` Core command extension seam exists through
  `Command::Extension`.
- [x] `Ecosystem` Shared extension manifest and registry contract in
  `filer-ecosystem`.
- [ ] `Ecosystem` Support one complete git decoration vertical slice before
  broad extension manager, marketplace, package UI, or plugin-rendered panels.
- [ ] `Ecosystem` Treat the first extension host as trusted in-process or a
  trusted core add-on, not a marketplace-safe plugin runtime.
- [ ] `Ecosystem` App-visible extension/plugin manifest UI.
- [ ] `Ecosystem` Extension manager UI.
- [ ] `Ecosystem` Command palette extension points.
- [ ] `Ecosystem` Preview provider extension points.
- [ ] `Ecosystem` Metadata provider extension points.
- [ ] `Ecosystem` Converter/action extension points.
- [ ] `Ecosystem` File row decoration and status badge rendering for
  extension-produced semantic state.
- [ ] `Ecosystem` Postpone arbitrary extension tabs, popups, panels, and layout
  control until row decorations and actions are proven.
- [ ] `Ecosystem` Theme and icon-pack extension points.
- [ ] `Ecosystem` Capability declaration and conflict detection.
- [ ] `Ecosystem` Sandboxed/scoped filesystem access for extensions.
- [ ] `Future` Extension marketplace or curated registry.

## Themes, Layout, And Accessibility

- [x] `MVP` Light, dark, and automatic theme modes.
- [x] `MVP` Files-style app palette and compact controls.
- [ ] `Polish` Persist theme, density, and layout settings.
- [ ] `Polish` Persist sidebar and preview panel widths.
- [ ] `Polish` High-contrast theme.
- [ ] `Polish` Keyboard shortcut remapping.
- [ ] `Polish` Accessible labels for controls and file rows.
- [ ] `Ecosystem` Serializable theme token format.
- [ ] `Ecosystem` Custom themes.
- [ ] `Ecosystem` Swappable icon packs.

## Reliability And Performance

- [x] `MVP` Active-directory watcher is hooked to the app.
- [x] `MVP` Watcher refresh bypasses stale directory cache.
- [x] `MVP` Same-folder reloads preserve selection when selected nodes still
  exist.
- [ ] `MVP` Recoverable error UI with clear actions.
- [ ] `MVP` Window-aware overlay placement for context menus and modals.
- [ ] `Polish` Large-directory list virtualization.
- [ ] `Polish` Align large-directory rendering with the core loading contract
  before rewriting the file list deeply.
- [x] `Polish` Non-blocking preview and thumbnail loading under rapid
  selection.
- [ ] `Polish` Structured log controls for app/core debugging.
- [ ] `Polish` Automated UI smoke checks for core workflows.
- [ ] `Power` Operation telemetry for long-running file operations.

## Validation Checklist

Run these before calling a UI milestone complete:

```bash
cargo fmt
cargo check -p filer-app
cargo test -p filer-app
cargo test -p filer-core scanner_cache_tests -- --nocapture
cargo test -p filer-core watcher -- --nocapture
```

Manual checks:

- [ ] Navigate by sidebar, address bar, back, forward, and up controls.
- [ ] Create a file externally in the current directory and confirm it appears.
- [ ] Sort and group a folder, then refresh and confirm the pipeline remains
  active.
- [ ] Right-click rows near each viewport edge and confirm the menu is usable.
- [ ] Create, rename, copy, cut, paste, and delete a test file/folder.
- [ ] Select files and confirm preview/status state stays coherent after
  refresh.
- [ ] Confirm the app still feels like a fast file manager, not a heavy IDE.
- [ ] Confirm extension output is rendered from semantic core events, not from
  extension-owned desktop widgets.
