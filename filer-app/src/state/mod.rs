pub mod clipboard;
pub mod selection;

use std::collections::HashMap;
use std::path::PathBuf;

use filer_core::model::node::NodeId;
use filer_core::modules::navigation::navigator::NavState;
use filer_core::pipeline::GroupedNodes;
use filer_core::{FileNode, PreviewData};

pub use clipboard::ClipboardState;
pub use selection::{SelectMode, SelectionState};

/// Active tab in the right-side details panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelTab {
    #[default]
    Preview,
    Details,
}

/// State for an in-progress rename.
#[derive(Debug, Clone)]
pub struct RenameState {
    pub node: NodeId,
    pub current_name: String,
}

/// State for an in-progress folder creation.
#[derive(Debug, Clone)]
pub struct CreateFolderState {
    pub name: String,
}

/// State for an in-progress file creation.
#[derive(Debug, Clone)]
pub struct CreateFileState {
    pub name: String,
}

/// State for a visible context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub node: NodeId,
    pub position: iced::Point,
}

/// State for an ongoing file operation.
#[derive(Debug, Clone)]
pub struct OperationProgressState {
    pub label: String,
    pub done: usize,
    pub total: usize,
}

/// All mutable GUI state for the application.
#[derive(Debug)]
pub struct AppState {
    // ── Navigation ──────────────────────────────────────────────────
    pub current_path: PathBuf,
    pub address_input: String,
    pub nav: Option<NavState>,
    pub current_watch: Option<NodeId>,
    pub watch_refresh_pending: bool,

    // ── File list ───────────────────────────────────────────────────
    pub groups: GroupedNodes,
    pub selection: SelectionState,

    // ── Search ──────────────────────────────────────────────────────
    pub search_query: String,
    /// Pending debounce value (set on each keystroke, consumed on Tick).
    pub search_pending: Option<String>,
    pub search_results: Option<Vec<FileNode>>,
    pub search_active: bool,
    pub search_result_count: usize,

    // ── Clipboard ───────────────────────────────────────────────────
    pub clipboard: Option<ClipboardState>,

    // ── Preview ─────────────────────────────────────────────────────
    pub preview_visible: bool,
    pub panel_tab: PanelTab,
    pub preview_node: Option<NodeId>,
    pub preview_data: Option<PreviewData>,

    // ── Overlays ────────────────────────────────────────────────────
    pub context_menu: Option<ContextMenuState>,
    pub last_pointer: iced::Point,
    pub rename_state: Option<RenameState>,
    pub create_folder_state: Option<CreateFolderState>,
    pub create_file_state: Option<CreateFileState>,
    pub operation_progress: Option<OperationProgressState>,

    // ── Sidebar ─────────────────────────────────────────────────────
    pub bookmarks: Vec<PathBuf>,
    pub recent_paths: Vec<PathBuf>,
    pub places: Vec<(String, PathBuf)>,

    // ── Thumbnails (NodeId → PNG bytes for loaded thumbs) ──────────
    pub thumbnails: HashMap<NodeId, Vec<u8>>,

    // ── Error display ───────────────────────────────────────────────
    pub error_message: Option<String>,
}

impl AppState {
    pub fn new(preview_visible: bool, bookmarks: Vec<PathBuf>, recent_paths: Vec<PathBuf>) -> Self {
        let places = build_places();
        Self {
            current_path: home_dir(),
            address_input: home_dir().display().to_string(),
            nav: None,
            current_watch: None,
            watch_refresh_pending: false,
            groups: GroupedNodes {
                groups: vec![],
                total_count: 0,
            },
            selection: SelectionState::new(),
            search_query: String::new(),
            search_pending: None,
            search_results: None,
            search_active: false,
            search_result_count: 0,
            clipboard: None,
            preview_visible,
            panel_tab: PanelTab::Preview,
            preview_node: None,
            preview_data: None,
            context_menu: None,
            last_pointer: iced::Point::ORIGIN,
            rename_state: None,
            create_folder_state: None,
            create_file_state: None,
            operation_progress: None,
            bookmarks,
            recent_paths,
            places,
            thumbnails: HashMap::new(),
            error_message: None,
        }
    }

    /// Add `path` to the top of recents, keeping it unique and bounded.
    pub fn add_recent_path(&mut self, path: &PathBuf, max: usize) {
        self.recent_paths.retain(|p| p != path);
        self.recent_paths.insert(0, path.clone());
        if self.recent_paths.len() > max {
            self.recent_paths.truncate(max);
        }
    }

    /// Flat list of all currently visible nodes (search results or directory listing).
    pub fn visible_nodes(&self) -> Vec<FileNode> {
        if let Some(results) = &self.search_results {
            return results.clone();
        }
        self.groups
            .groups
            .iter()
            .flat_map(|g| g.nodes.clone())
            .collect()
    }

    /// Ordered NodeIds of all visible nodes (for range selection).
    pub fn visible_ids(&self) -> Vec<NodeId> {
        self.visible_nodes().iter().map(|n| n.id).collect()
    }

    /// Total size of all selected nodes.
    pub fn selected_size(&self) -> u64 {
        self.visible_nodes()
            .iter()
            .filter(|n| self.selection.contains(n.id))
            .map(|n| n.size)
            .sum()
    }

    pub fn retain_selection_for_visible_nodes(&mut self) {
        let visible = self.visible_ids();
        self.selection.retain(&visible);
    }
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

fn build_places() -> Vec<(String, PathBuf)> {
    let mut places = Vec::new();
    if let Some(p) = dirs::home_dir() {
        places.push(("Home".to_string(), p));
    }
    if let Some(p) = dirs::desktop_dir() {
        places.push(("Desktop".to_string(), p));
    }
    if let Some(p) = dirs::document_dir() {
        places.push(("Documents".to_string(), p));
    }
    if let Some(p) = dirs::download_dir() {
        places.push(("Downloads".to_string(), p));
    }
    if let Some(p) = dirs::picture_dir() {
        places.push(("Pictures".to_string(), p));
    }
    if let Some(p) = dirs::video_dir() {
        places.push(("Videos".to_string(), p));
    }
    if let Some(p) = dirs::audio_dir() {
        places.push(("Music".to_string(), p));
    }
    #[cfg(windows)]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = PathBuf::from(&drive);
            if path.exists() {
                places.push((format!("Drive {}", letter as char), path));
            }
        }
    }

    #[cfg(not(windows))]
    {
        places.push(("Root".to_string(), PathBuf::from("/")));
    }
    places
}
