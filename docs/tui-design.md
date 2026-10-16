# Terminal interface design

Browsing a directory and inspecting a file should fit in the same terminal without filling every row with metadata. Filer's terminal interface uses scrollable directory columns and a contextual inspector. This document defines the intended interaction contract; the [terminal task map](../.tasks/tui/TUI-001-build-a-linux-terminal-file-manager-over-public-core-contracts.md) owns delivery scope and sequencing.

## Browse through columns

Use Miller columns to show successive levels of one folder hierarchy. Columns belong to one browsing context and one Session. They are not independent source and destination panes.

Keep the active directory list readable, with at most three visible columns: parent context, active directory, and inspector. Moving deeper shifts the visible columns horizontally. Older ancestors remain accessible through parent navigation and the path line rather than accumulating narrower columns.

Each directory list scrolls vertically. Remember its cursor Location and scroll position when entering a child and returning to its parent. Reconcile remembered positions against refreshed entries by Location rather than row number. If an entry disappears, place the cursor on a surviving neighbor or show the empty state.

Directory columns show one entry per line. Use a trailing slash to distinguish folders, one cursor highlight in the active list, and a separate marker for files selected for operations. Moving the cursor does not change that selection.

## Inspect the current item

The rightmost column follows the active list's cursor. Reuse this space for content and metadata so inspecting a file does not require another permanent panel.

| Inspector mode | Content |
| --- | --- |
| Contextual, folder under cursor | Child entries without changing the active directory |
| Contextual, file under cursor | Content preview |
| Details | Full name, type, size, permissions, and modification time where available |
| Expanded preview | A full-screen reader for the current item's preview |

Keep details mode selected while the cursor moves until the user closes it. A folder's child list becomes the active directory only after explicit navigation. Merely displaying children must not change navigation history.

Show loading, unavailable content, and recoverable errors inside the inspector. An unsupported preview falls back to available metadata with a visible explanation. Preview failure leaves the active list usable.

## Make focus explicit

The cursor identifies the item being inspected. Selection identifies entries targeted by a file operation. Keyboard focus identifies the surface receiving movement and scrolling input. Keep these three states separate.

Preview updates do not take focus from the list. The inspector receives focus only through an explicit action and shows a focus indicator. Its scroll position is independent of directory scrolling.

| Input | Directory list | Focused side inspector | Expanded reader |
| --- | --- | --- | --- |
| Up / Down | Move the cursor and scroll its list | Scroll inspector content | Scroll reader content |
| Left | Navigate to the parent | No action | No action |
| Right / Enter | Enter the folder under the cursor | No action | No action |
| Tab | Focus the inspector when visible | Return focus to the list | No action |
| `i` | Toggle details for the current item | Toggle details | No action |
| `p` | Expand the current preview | Expand the current preview | No action |
| Esc | Close details and return to contextual inspection | Return focus to the list | Restore the browsing view |
| `?` | Show available commands | Show available commands | Show available commands |

Entering a folder is distinct from opening a file in another program. The command help exposes the file-opening action separately. Expanded reading preserves the directory cursor, selected entries, list scroll, and side inspector state; Esc restores that browsing view.

## Keep the layout sparse

Use one path line, thin vertical separators, and one compact status line. The active list receives the strongest emphasis. Keep metadata in the inspector and use text markers as well as color for focus and selection.

The status line shows context-relevant key hints and loading or error state. Full command help appears on demand. Report a partial directory listing as partial rather than presenting its loaded row count as a final total.

When space becomes constrained, hide ancestor columns first. If the active list and inspector cannot both remain readable, show the list alone and open preview or details full screen on request. Resizing preserves browsing state and returns focus to the list if its focused inspector is hidden. Truncation uses terminal display width; the inspector provides the full filename.

## Update state before rendering

Use an Elm-style model, update, and view split. Keyboard input, core events, resize events, and effect deadlines become messages. A renderer-independent update function changes client state and returns explicit effects. A separate runner performs core requests and terminal work; rendering only reads state and draws the visible range.

The client owns layout, keyboard focus, directory cursor positions, scroll offsets, inspector mode, and displayed results. Core remains the authority for navigation, listing order, filtering, metadata, previews, and file operations. Keep Ratatui and TachyonFX outside the state model, and consume core events through one bridge. Follow the [state ownership rules](architecture/state-ownership.md) and [read-request lifecycle](adr/0001-core-runtime-lifecycle.md#read-requests).

Correlate directory, preview, and metadata results independently. Moving the cursor invalidates obsolete inspector requests without cancelling unrelated listing work. Request cancellation is paired with identity checks so a late result cannot replace the current item's content. Hiding the inspector releases its disposable work without affecting accepted file operations.

Preview loading and preparation stay outside input processing and rendering. Directory views request pages near their visible range and do not load every ancestor just to draw a column. Effects never delay input or core event consumption. Idle rendering waits for input, core events, or active effect deadlines.

Keep the terminal model local to the client until another client demonstrates shared behavior. Splitting state updates from rendering provides a testing seam without requiring a shared client framework.

## Verify the interaction contract

Write state-transition tests before implementing behavior, using the repository's external test style. Cover entering and returning from folders, cursor restoration after refresh or removal, selection independent of focus, inspector mode changes, preview errors, and stale results after rapid navigation.

Use Ratatui's test backend for visible columns, focus indicators, Unicode width, and narrow layouts. Use Linux PTY checks for resize, reader exit, and terminal restoration. Expanding and closing a preview must preserve the same browsing state, and delayed preview work must not prevent list input from being processed.

## Pattern references

- [Ranger](https://github.com/ranger/ranger) demonstrates column browsing with file previews.
- [Yazi's layout configuration](https://yazi-rs.github.io/docs/configuration/yazi/#ratio) separates parent, current, and preview columns.
- [Ratatui's Elm Architecture guide](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/) describes the model, update, and view pattern.
