use std::path::PathBuf;

use filer_core::api::events::OperationKind;
use filer_core::model::node::NodeId;

use crate::state::SelectMode;

/// Serializable sort field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    Modified,
    Kind,
}

/// Sort direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// All messages that drive application state changes.
#[derive(Debug, Clone)]
pub enum Message {
    // ── Core events ─────────────────────────────────────────────────
    CoreEvent(filer_core::Event),

    // ── Navigation ──────────────────────────────────────────────────
    Navigate(PathBuf),
    NavigateBack,
    NavigateForward,
    NavigateUp,
    Refresh,

    // ── File list ───────────────────────────────────────────────────
    OpenNode(NodeId),
    SelectNode(NodeId, SelectMode),
    SelectAll,

    // ── Search ──────────────────────────────────────────────────────
    SearchInput(String),
    SearchCommit,
    SearchClear,

    // ── Context menu ────────────────────────────────────────────────
    ShowContextMenu(NodeId, iced::Point),
    HideContextMenu,

    // ── Clipboard & operations ───────────────────────────────────────
    CopySelected,
    CutSelected,
    Paste,
    DeleteSelected { trash: bool },
    RenameStart(NodeId),
    RenameInput(String),
    RenameCommit,
    RenameCancel,
    CreateFolderStart,
    CreateFolderInput(String),
    CreateFolderCommit,
    CreateFolderCancel,

    // ── Preview ─────────────────────────────────────────────────────
    TogglePreview,

    // ── Sort ────────────────────────────────────────────────────────
    SortBy(SortField, SortDir),

    // ── Bookmarks ───────────────────────────────────────────────────
    AddBookmark,
    RemoveBookmark(PathBuf),

    // ── Operations progress ─────────────────────────────────────────
    OperationProgress(OperationKind, usize, usize),

    // ── Debounce tick ───────────────────────────────────────────────
    /// Fired after 150 ms; commits a pending search query.
    Tick,
}
