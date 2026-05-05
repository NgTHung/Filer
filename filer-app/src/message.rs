use std::path::PathBuf;

use filer_core::api::events::OperationKind;
use filer_core::model::node::NodeId;

use crate::config::ThemeMode;
use crate::state::{PanelTab, SelectMode};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMode {
    None,
    Extension,
    Date,
    Size,
    FirstLetter,
}

/// All messages that drive application state changes.
#[derive(Debug, Clone)]
pub enum Message {
    // ── Core events ─────────────────────────────────────────────────
    CoreEvent(filer_core::Event),

    // ── Navigation ──────────────────────────────────────────────────
    Navigate(PathBuf),
    AddressInputChanged(String),
    AddressSubmit,
    NavigateBack,
    NavigateForward,
    NavigateUp,
    Refresh,

    // ── File list ───────────────────────────────────────────────────
    ActivateNode(NodeId),
    OpenContextMenu(NodeId),
    PointerMoved(iced::Point),
    OpenNode(NodeId),
    OpenSelected,
    SelectNode(NodeId, SelectMode),
    SelectAll,

    // ── Search ──────────────────────────────────────────────────────
    SearchInput(String),
    SearchCommit,
    SearchClear,

    // ── Context menu ────────────────────────────────────────────────
    ShowContextMenu(NodeId, iced::Point),
    HideContextMenu,
    HideContextMenuOnly,

    // ── Clipboard & operations ───────────────────────────────────────
    CopySelected,
    CutSelected,
    Paste,
    DeleteSelected {
        trash: bool,
    },
    RenameStart(NodeId),
    RenameInput(String),
    RenameCommit,
    RenameCancel,
    CreateFolderStart,
    CreateFolderInput(String),
    CreateFolderCommit,
    CreateFolderCancel,
    CreateFileStart,
    CreateFileInput(String),
    CreateFileCommit,
    CreateFileCancel,

    // ── Preview ─────────────────────────────────────────────────────
    TogglePreview,
    SetPanelTab(PanelTab),

    // ── Theme ───────────────────────────────────────────────────────
    SetThemeMode(ThemeMode),

    // ── Sort ────────────────────────────────────────────────────────
    SortBy(SortField, SortDir),
    GroupBy(GroupMode),

    // ── Bookmarks ───────────────────────────────────────────────────
    AddBookmark,
    RemoveBookmark(PathBuf),

    // ── Operations progress ─────────────────────────────────────────
    OperationProgress(OperationKind, usize, usize),

    // ── Error display ───────────────────────────────────────────────
    DismissError,

    // ── Watch refresh debounce ──────────────────────────────────────
    WatchRefreshDue,

    // ── Debounce tick ───────────────────────────────────────────────
    /// Fired after 150 ms; commits a pending search query.
    Tick,
}
