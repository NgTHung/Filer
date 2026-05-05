# filer-app Roadmap

This roadmap tracks the desktop frontend only. The root roadmap covers
`filer-core` and broader product milestones; this file focuses on making the
Iced app pleasant, usable, and ready for day-to-day local file management.

## Product Direction

The app should feel like a practical Rust file manager with a Files-style visual
language and Xplorer-style workflow intent.

- Simple and elegant: quiet palette, clear spacing, readable rows.
- Work-focused: dense enough to scan directories quickly.
- Actionable: right-click, search, preview, topbar, status bar, and Quick Access
  should all be useful, not placeholders.
- Honest: ship polished local-first behavior before presenting advanced features
  as complete.

## Phase 1: Usable Polish

Goal: make the current app look and feel good enough to use while the next
feature phase is being built.

- Add a consistent Files-style light/dark palette.
- Replace default-looking buttons with compact app-specific controls.
- Polish the topbar: navigation, refresh, new folder, bookmark, paste, panel
  toggle, breadcrumb, and search.
- Improve Quick Access, bookmarks, recent folders, and theme controls in the
  sidebar.
- Keep the details-list view as the primary file surface and improve row
  spacing, hover state, selected state, icons, column alignment, and empty
  states.
- Make the right-click context menu usable and positioned near the pointer.
- Keep the Preview/Details panel visible and make loading, unsupported, image,
  text, code, archive, binary, and folder states clear.
- Improve the bottom status bar so it reports item count, selection count/size,
  sort state, and search state.

Completion criteria:

- `cargo check --workspace` passes.
- The app can be launched with `cargo run -p filer-app`.
- Navigation, search, selection, preview, right-click actions, and theme changes
  are usable without obvious visual breakage.

## Phase 2: Interaction Completion

Goal: make core file-manager interactions predictable and complete.

- Polish keyboard navigation: arrows, Enter, Backspace, Alt+Left/Right, Delete,
  Ctrl+A, Ctrl+C, Ctrl+X, Ctrl+V, and `/`.
- Complete multi-selection behavior, including range and toggle selection.
- Improve inline rename and create-folder flows.
- Clamp and dismiss context menus correctly.
- Add better recoverable error displays for failed operations.
- Ensure operation completion refreshes the right directory and preserves useful
  UI state where possible.

Completion criteria:

- Common keyboard and mouse workflows match user expectations.
- Failed operations produce clear, recoverable messages.
- Multi-selection works consistently across list, context menu, and status bar.

## Phase 3: File-Manager Ergonomics

Goal: make the app comfortable for repeated real use.

- Persist sidebar and preview panel widths.
- Persist view preferences such as sort field, sort direction, panel visibility,
  and theme mode.
- Add Quick Access management for bookmarks and pinned folders.
- Add a clearer operation progress tray for longer copy/move/delete work.
- Add drag and drop for internal move/copy workflows.
- Add a simple tile or icon view after the details-list mode is solid.

Completion criteria:

- Layout and view preferences survive restart.
- Quick Access can be managed without editing config files.
- Long operations are visible and do not make the app feel frozen.

## Phase 4: Preview And Metadata Depth

Goal: make the right panel genuinely useful for inspecting files.

- Add thumbnail loading for images and common previewable file types.
- Add thumbnail disk cache keyed by stable file identity or content hash.
- Expand the Details tab with richer metadata from `filer-core`.
- Improve text and code previews with better truncation, monospace layout, and
  syntax highlighting where available.
- Improve archive, document, media, and unsupported-file states.

Completion criteria:

- Selecting common files gives a useful preview or a clear reason why preview is
  unavailable.
- Thumbnail loading is lazy and does not block the main UI.
- Details are useful for deciding what a file is without opening it elsewhere.

## Phase 5: Advanced Workflows

Goal: add power-user workflows after the local-first app is stable.

- Tabs and split-pane browsing.
- Command palette for navigation and file actions.
- Operation queue and history.
- Remote/provider entry points for capabilities already present in `filer-core`.
- More configurable file views and sidebar sections.

Completion criteria:

- Advanced workflows build on the stable local file manager instead of replacing
  it.
- Remote and virtual providers are surfaced only when their core behavior is
  reliable enough for the UI.
