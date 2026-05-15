use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use filer_core::model::node::{NodeId, NodeKind};
use filer_core::model::session::SessionId;
use filer_core::{Command, FilerCore, OperationId, PreviewData, RequestId};
use iced::futures::Stream;
use iced::keyboard::{Key, Modifiers};
use iced::theme::Mode;
use iced::{Color, Element, Subscription, Task, Theme};
use tracing::{debug, trace};

use crate::config::{Config, ThemeMode};
use crate::message::{GroupMode, Message, SortDir, SortField};
use crate::state::{
    AppState, ClipboardState, ContextMenuState, CreateFileState, CreateFolderState,
    OperationProgressState, RenameState, SelectMode,
};
use crate::views;

const SEARCH_DEBOUNCE_MS: u64 = 150;
const WATCH_REFRESH_DEBOUNCE_MS: u64 = 200;
const MAX_RECENT_PATHS: usize = 12;

#[derive(Clone)]
struct CoreRx(flume::Receiver<filer_core::Event>);

impl Hash for CoreRx {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u64.hash(state);
    }
}

fn core_event_stream(
    rx: &CoreRx,
) -> std::pin::Pin<Box<dyn Stream<Item = Message> + Send + 'static>> {
    let rx = rx.0.clone();
    Box::pin(iced::futures::stream::unfold(rx, |rx| async move {
        match rx.recv_async().await {
            Ok(event) => Some((Message::CoreEvent(event), rx)),
            Err(_) => None,
        }
    }))
}

fn map_keyboard(event: iced::keyboard::Event) -> Option<Message> {
    use iced::keyboard::Event::KeyPressed;
    let KeyPressed { key, modifiers, .. } = event else {
        return None;
    };

    if modifiers.contains(Modifiers::CTRL) {
        return match &key {
            Key::Character(c) if c.as_str() == "a" => Some(Message::SelectAll),
            Key::Character(c) if c.as_str() == "c" => Some(Message::CopySelected),
            Key::Character(c) if c.as_str() == "x" => Some(Message::CutSelected),
            Key::Character(c) if c.as_str() == "v" => Some(Message::Paste),
            Key::Character(c) if c.as_str() == "f" => Some(Message::SearchInput(String::new())),
            _ => None,
        };
    }

    if modifiers.contains(Modifiers::ALT) {
        return match &key {
            Key::Named(iced::keyboard::key::Named::ArrowLeft) => Some(Message::NavigateBack),
            Key::Named(iced::keyboard::key::Named::ArrowRight) => Some(Message::NavigateForward),
            _ => None,
        };
    }

    match &key {
        Key::Named(iced::keyboard::key::Named::Backspace) => Some(Message::NavigateUp),
        Key::Named(iced::keyboard::key::Named::Escape) => Some(Message::HideContextMenu),
        Key::Named(iced::keyboard::key::Named::Delete) => {
            Some(Message::DeleteSelected { trash: true })
        }
        Key::Named(iced::keyboard::key::Named::Enter) => Some(Message::OpenSelected),
        Key::Character(c) if c.as_str() == "/" => Some(Message::SearchInput(String::new())),
        _ => None,
    }
}

pub struct App {
    core: Arc<FilerCore>,
    session: SessionId,
    state: AppState,
    sort_field: SortField,
    sort_dir: SortDir,
    group_mode: GroupMode,
    config: Config,
    theme_mode: ThemeMode,
}

impl App {
    pub fn new(core: Arc<FilerCore>) -> (Self, Task<Message>) {
        let config = Config::load();
        let _ = core.send(Command::Handshake);

        let state = AppState::new(
            config.preview_visible,
            config.bookmarks.clone(),
            config.recent_paths.clone(),
        );

        let app = Self {
            core,
            session: SessionId::DEFAULT,
            state,
            sort_field: SortField::Name,
            sort_dir: SortDir::Asc,
            group_mode: GroupMode::None,
            theme_mode: config.theme_mode.clone(),
            config,
        };
        (app, Task::none())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard = iced::keyboard::listen().filter_map(map_keyboard);
        let pointer = iced::event::listen_with(map_pointer_event);
        Subscription::batch([
            Subscription::run_with(CoreRx(self.core.event_receiver()), core_event_stream),
            keyboard,
            pointer,
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CoreEvent(event) => self.handle_core_event(event),

            Message::Navigate(path) => {
                debug!(path = %path.display(), session = %self.session, "ui -> core navigate");
                self.state.search_active = false;
                self.state.search_results = None;
                self.state.search_result_count = 0;
                self.state.search_query.clear();
                self.state.selection.clear();
                self.state.preview_data = None;
                self.state.context_menu = None;
                self.state.address_input = path.display().to_string();
                let _ = self.core.send(Command::Navigate {
                    path,
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }
            Message::AddressInputChanged(value) => {
                self.state.address_input = value;
                Task::none()
            }
            Message::AddressSubmit => {
                let typed = self.state.address_input.trim();
                if typed.is_empty() {
                    return Task::none();
                }
                let path = PathBuf::from(typed);
                self.update(Message::Navigate(path))
            }
            Message::NavigateBack => {
                debug!(session = %self.session, "ui -> core navigate back");
                let _ = self.core.send(Command::NavigateBack {
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }
            Message::NavigateForward => {
                debug!(session = %self.session, "ui -> core navigate forward");
                let _ = self.core.send(Command::NavigateForward {
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }
            Message::NavigateUp => {
                debug!(session = %self.session, "ui -> core navigate up");
                let _ = self.core.send(Command::NavigateUp {
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }
            Message::Refresh => {
                debug!(session = %self.session, "ui -> core refresh");
                self.state.context_menu = None;
                let _ = self.core.send(Command::Refresh {
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }

            Message::ActivateNode(id) => self.activate_node(id),
            Message::OpenContextMenu(id) => {
                if !self.state.selection.contains(id) {
                    self.state.selection.select_one(id);
                }
                self.state.context_menu = Some(ContextMenuState {
                    node: id,
                    position: self.state.last_pointer,
                });
                Task::none()
            }
            Message::PointerMoved(point) => {
                self.state.last_pointer = point;
                Task::none()
            }
            Message::OpenNode(id) => self.open_node(id),
            Message::OpenSelected => {
                let selected = self.state.selection.ids();
                if selected.len() == 1 {
                    self.open_node(selected[0])
                } else {
                    Task::none()
                }
            }
            Message::SelectNode(id, mode) => {
                self.select_node(id, mode);
                Task::none()
            }
            Message::SelectAll => {
                let ids = self.state.visible_ids();
                self.state.selection.select_all(&ids);
                Task::none()
            }

            Message::SearchInput(query) => {
                self.state.search_query = query.clone();
                self.state.search_pending = Some(query);
                self.state.search_results = None;
                self.state.search_result_count = 0;
                Task::perform(
                    async { tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await },
                    |_| Message::Tick,
                )
            }
            Message::Tick => {
                if let Some(query) = self.state.search_pending.take() {
                    if query.is_empty() {
                        self.state.search_active = false;
                        self.state.search_results = None;
                        self.state.search_result_count = 0;
                    } else {
                        self.state.search_active = true;
                        let root = self
                            .core
                            .registry()
                            .register(self.state.current_path.clone());
                        let _ = self.core.send(Command::Search {
                            query,
                            root,
                            session: self.session,
                            request: RequestId::new(),
                        });
                    }
                }
                Task::none()
            }
            Message::SearchCommit => self.update(Message::Tick),
            Message::SearchClear => {
                self.state.search_query.clear();
                self.state.search_pending = None;
                self.state.search_active = false;
                self.state.search_results = None;
                self.state.search_result_count = 0;
                Task::none()
            }

            Message::ShowContextMenu(node, point) => {
                if !self.state.selection.contains(node) {
                    self.state.selection.select_one(node);
                }
                self.state.context_menu = Some(ContextMenuState {
                    node,
                    position: point,
                });
                Task::none()
            }
            Message::HideContextMenu => {
                self.state.context_menu = None;
                self.state.rename_state = None;
                self.state.create_folder_state = None;
                self.state.create_file_state = None;
                Task::none()
            }
            Message::HideContextMenuOnly => {
                self.state.context_menu = None;
                Task::none()
            }

            Message::CopySelected => {
                let ids = self.state.selection.ids();
                if !ids.is_empty() {
                    self.state.clipboard = Some(ClipboardState::copy(ids));
                }
                self.state.context_menu = None;
                Task::none()
            }
            Message::CutSelected => {
                let ids = self.state.selection.ids();
                if !ids.is_empty() {
                    self.state.clipboard = Some(ClipboardState::cut(ids));
                }
                self.state.context_menu = None;
                Task::none()
            }
            Message::Paste => {
                if let Some(cb) = self.state.clipboard.clone() {
                    if let Some(dest) = self.current_node_id() {
                        if cb.is_cut() {
                            let _ = self.core.send(Command::Move {
                                sources: cb.nodes,
                                destination: dest,
                                session: self.session,
                                request: RequestId::new(),
                                operation: OperationId::new(),
                            });
                            self.state.clipboard = None;
                        } else {
                            let _ = self.core.send(Command::Copy {
                                sources: cb.nodes,
                                destination: dest,
                                session: self.session,
                                request: RequestId::new(),
                                operation: OperationId::new(),
                            });
                        }
                    }
                }
                self.state.context_menu = None;
                Task::none()
            }
            Message::DeleteSelected { trash } => {
                let ids = self.state.selection.ids();
                if !ids.is_empty() {
                    let _ = self.core.send(Command::Delete {
                        nodes: ids,
                        trash,
                        session: self.session,
                        request: RequestId::new(),
                        operation: OperationId::new(),
                    });
                }
                self.state.context_menu = None;
                Task::none()
            }
            Message::RenameStart(id) => {
                let name = self
                    .resolve(id)
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                    .unwrap_or_default();
                self.state.rename_state = Some(RenameState {
                    node: id,
                    current_name: name,
                });
                self.state.context_menu = None;
                Task::none()
            }
            Message::RenameInput(s) => {
                if let Some(r) = &mut self.state.rename_state {
                    r.current_name = s;
                }
                Task::none()
            }
            Message::RenameCommit => {
                if let Some(r) = self.state.rename_state.take() {
                    let _ = self.core.send(Command::Rename {
                        node: r.node,
                        new_name: r.current_name,
                        session: self.session,
                        request: RequestId::new(),
                        operation: OperationId::new(),
                    });
                }
                Task::none()
            }
            Message::RenameCancel => {
                self.state.rename_state = None;
                Task::none()
            }
            Message::CreateFolderStart => {
                self.state.create_folder_state = Some(CreateFolderState {
                    name: String::new(),
                });
                self.state.create_file_state = None;
                self.state.context_menu = None;
                Task::none()
            }
            Message::CreateFolderInput(s) => {
                if let Some(c) = &mut self.state.create_folder_state {
                    c.name = s;
                }
                Task::none()
            }
            Message::CreateFolderCommit => {
                if let Some(c) = self.state.create_folder_state.take() {
                    if !c.name.is_empty() {
                        if let Some(parent_id) = self.current_node_id() {
                            let _ = self.core.send(Command::CreateFolder {
                                parent: parent_id,
                                name: c.name,
                                session: self.session,
                                request: RequestId::new(),
                                operation: OperationId::new(),
                            });
                        }
                    }
                }
                Task::none()
            }
            Message::CreateFolderCancel => {
                self.state.create_folder_state = None;
                Task::none()
            }
            Message::CreateFileStart => {
                self.state.create_file_state = Some(CreateFileState {
                    name: String::new(),
                });
                self.state.create_folder_state = None;
                self.state.context_menu = None;
                Task::none()
            }
            Message::CreateFileInput(s) => {
                if let Some(c) = &mut self.state.create_file_state {
                    c.name = s;
                }
                Task::none()
            }
            Message::CreateFileCommit => {
                if let Some(c) = self.state.create_file_state.take() {
                    if !c.name.is_empty() {
                        if let Some(parent_id) = self.current_node_id() {
                            let _ = self.core.send(Command::CreateFile {
                                parent: parent_id,
                                name: c.name,
                                session: self.session,
                                request: RequestId::new(),
                                operation: OperationId::new(),
                            });
                        }
                    }
                }
                Task::none()
            }
            Message::CreateFileCancel => {
                self.state.create_file_state = None;
                Task::none()
            }

            Message::TogglePreview => {
                self.state.preview_visible = !self.state.preview_visible;
                self.config.preview_visible = self.state.preview_visible;
                self.config.save();
                Task::none()
            }
            Message::SetPanelTab(tab) => {
                self.state.panel_tab = tab;
                Task::none()
            }
            Message::SetThemeMode(mode) => {
                self.theme_mode = mode.clone();
                self.config.theme_mode = mode;
                self.config.save();
                Task::none()
            }

            Message::AddBookmark => {
                let p = self.state.current_path.clone();
                if !self.state.bookmarks.contains(&p) {
                    self.state.bookmarks.push(p);
                    self.config.bookmarks = self.state.bookmarks.clone();
                    self.config.save();
                }
                Task::none()
            }
            Message::RemoveBookmark(path) => {
                self.state.bookmarks.retain(|b| *b != path);
                self.config.bookmarks = self.state.bookmarks.clone();
                self.config.save();
                Task::none()
            }

            Message::SortBy(field, dir) => {
                self.sort_field = field.clone();
                self.sort_dir = dir.clone();
                self.rescan_with_current_pipeline();
                Task::none()
            }
            Message::GroupBy(group) => {
                self.group_mode = group;
                self.rescan_with_current_pipeline();
                Task::none()
            }

            Message::OperationProgress(operation_id, kind, done, total) => {
                use filer_core::api::events::OperationKind;
                let label = match kind {
                    OperationKind::Copy => "Copying",
                    OperationKind::Move => "Moving",
                    OperationKind::Delete => "Deleting",
                    OperationKind::Rename => "Renaming",
                    OperationKind::CreateFolder => "Creating folder",
                    OperationKind::CreateFile => "Creating file",
                }
                .to_string();
                self.state.operation_progress = Some(OperationProgressState {
                    operation_id,
                    label,
                    done,
                    total,
                });
                Task::none()
            }

            Message::DismissError => {
                self.state.error_message = None;
                Task::none()
            }
            Message::WatchRefreshDue => {
                self.state.watch_refresh_pending = false;
                debug!(session = %self.session, "ui -> core refresh from watcher");
                let _ = self.core.send(Command::Refresh {
                    session: self.session,
                    request: RequestId::new(),
                });
                Task::none()
            }
        }
    }

    fn handle_core_event(&mut self, event: filer_core::Event) -> Task<Message> {
        trace!(event = ?event, "core -> ui event");
        match event {
            filer_core::Event::DirectoryLoaded {
                parent,
                path,
                groups,
                session,
                ..
            } => {
                if session != self.session {
                    trace!(event_session = %session, app_session = %self.session, "ignoring DirectoryLoaded for inactive session");
                    return Task::none();
                }
                let same_path = self.state.current_path == path;
                self.state.current_path = path.clone();
                self.state.address_input = path.display().to_string();
                self.state.groups = groups;
                if same_path {
                    self.state.retain_selection_for_visible_nodes();
                } else {
                    self.state.selection.clear();
                }
                self.state.operation_progress = None;
                self.state.add_recent_path(&path, MAX_RECENT_PATHS);
                self.config.recent_paths = self.state.recent_paths.clone();
                self.config.save();
                self.ensure_current_watch(parent);
            }
            filer_core::Event::SearchResults {
                matches, complete, ..
            } => {
                let existing = self.state.search_results.get_or_insert_with(Vec::new);
                existing.extend(matches);
                self.state.search_result_count = existing.len();
                if complete {
                    self.state.search_active = false;
                }
            }
            filer_core::Event::PreviewReady { node, preview, .. } => {
                if let PreviewData::Image { ref data, .. } = preview {
                    self.state.thumbnails.insert(node, data.clone());
                }
                if Some(node) == self.state.preview_node {
                    self.state.preview_data = Some(preview);
                }
            }
            filer_core::Event::OperationComplete { .. } => {
                self.state.operation_progress = None;
                let _ = self.core.send(Command::Refresh {
                    session: self.session,
                    request: RequestId::new(),
                });
            }
            filer_core::Event::OperationProgress {
                operation_id,
                operation,
                items_done,
                total_items,
                ..
            } => {
                return self.update(Message::OperationProgress(
                    operation_id,
                    operation,
                    items_done,
                    total_items,
                ));
            }
            filer_core::Event::SessionCreated(id) => {
                self.session = id;
                let home = self.state.current_path.clone();
                debug!(session = %id, home = %home.display(), "core session created, navigating home");
                let _ = self.core.send(Command::Navigate {
                    path: home,
                    session: id,
                    request: RequestId::new(),
                });
            }
            filer_core::Event::CurrentNavigateState { state, .. } => {
                self.state.nav = Some(state);
            }
            filer_core::Event::Error { message, .. } => {
                self.state.error_message = Some(message);
            }
            filer_core::Event::FsChanged {
                node,
                kind,
                session,
            } => {
                if session != self.session {
                    trace!(event_session = %session, app_session = %self.session, "ignoring FsChanged for inactive session");
                    return Task::none();
                }

                debug!(node = ?node, kind = ?kind, session = %session, "core -> ui fs changed");
                if !self.state.watch_refresh_pending {
                    self.state.watch_refresh_pending = true;
                    return Task::perform(
                        async {
                            tokio::time::sleep(Duration::from_millis(WATCH_REFRESH_DEBOUNCE_MS))
                                .await;
                        },
                        |_| Message::WatchRefreshDue,
                    );
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        views::view(
            &self.state,
            &self.sort_field,
            &self.sort_dir,
            &self.group_mode,
            &self.theme_mode,
        )
    }

    pub fn theme(&self) -> Theme {
        let light = Theme::custom(
            "Filer Light",
            iced::theme::Palette {
                background: Color::from_rgb8(0xF7, 0xF9, 0xFC),
                text: Color::from_rgb8(0x1F, 0x29, 0x37),
                primary: Color::from_rgb8(0x25, 0x63, 0xEB),
                success: Color::from_rgb8(0x16, 0xA3, 0x4A),
                warning: Color::from_rgb8(0xB4, 0x53, 0x09),
                danger: Color::from_rgb8(0xDC, 0x26, 0x26),
            },
        );
        let dark = Theme::custom(
            "Filer Dark",
            iced::theme::Palette {
                background: Color::from_rgb8(0x15, 0x17, 0x1A),
                text: Color::from_rgb8(0xE8, 0xEA, 0xED),
                primary: Color::from_rgb8(0x4C, 0x8D, 0xFF),
                success: Color::from_rgb8(0x34, 0xD3, 0x99),
                warning: Color::from_rgb8(0xF5, 0x9E, 0x0B),
                danger: Color::from_rgb8(0xF8, 0x71, 0x71),
            },
        );
        match self.theme_mode {
            ThemeMode::Auto => <Theme as iced::theme::Base>::default(Mode::None),
            ThemeMode::Light => light,
            ThemeMode::Dark => dark,
        }
    }

    fn activate_node(&mut self, id: NodeId) -> Task<Message> {
        let already_selected = self.state.selection.contains(id);
        if already_selected {
            self.open_node(id)
        } else {
            self.select_node(id, SelectMode::Single);
            Task::none()
        }
    }

    fn open_node(&mut self, id: NodeId) -> Task<Message> {
        if let Some(path) = self.resolve(id) {
            if path.is_dir() {
                return self.update(Message::Navigate(path));
            }
        }
        Task::none()
    }

    fn select_node(&mut self, id: NodeId, mode: SelectMode) {
        self.state.context_menu = None;
        let all_ids = self.state.visible_ids();
        match mode {
            SelectMode::Single => self.state.selection.select_one(id),
            SelectMode::Toggle => self.state.selection.toggle(id),
            SelectMode::Range => self.state.selection.range_to(id, &all_ids),
        }

        if self.state.selection.len() == 1 {
            self.state.preview_node = Some(id);
            self.state.preview_data = None;
            let is_file = self
                .state
                .visible_nodes()
                .iter()
                .find(|n| n.id == id)
                .map(|n| matches!(n.kind, NodeKind::File { .. }))
                .unwrap_or(false);

            if is_file {
                let _ = self.core.send(Command::LoadPreview {
                    id,
                    options: None,
                    session: self.session,
                    request: RequestId::new(),
                });
            }
        }
    }

    fn resolve(&self, id: NodeId) -> Option<PathBuf> {
        self.core.registry().resolve(id)
    }

    fn current_node_id(&self) -> Option<NodeId> {
        Some(
            self.core
                .registry()
                .register(self.state.current_path.clone()),
        )
    }

    fn ensure_current_watch(&mut self, node: NodeId) {
        if self.state.current_watch == Some(node) {
            return;
        }

        if let Some(previous) = self.state.current_watch.replace(node) {
            debug!(node = ?previous, session = %self.session, "ui -> core unwatch previous directory");
            let _ = self.core.send(Command::Unwatch(previous));
        }

        debug!(node = ?node, session = %self.session, "ui -> core watch current directory");
        let _ = self.core.send(Command::Watch(node, self.session));
    }

    fn current_pipeline(&self) -> filer_core::pipeline::config::PipelineConfig {
        use filer_core::pipeline::config::PipelineConfig;
        use filer_core::pipeline::sort::{SortField as CoreField, SortOrder};

        let core_field = match self.sort_field {
            SortField::Name => CoreField::Name,
            SortField::Size => CoreField::Size,
            SortField::Modified => CoreField::Modified,
            SortField::Kind => CoreField::Type,
        };
        let order = match self.sort_dir {
            SortDir::Asc => SortOrder::Ascending,
            SortDir::Desc => SortOrder::Descending,
        };

        let mut pipeline = PipelineConfig::default().sort(core_field, order, true);
        pipeline = match self.group_mode {
            GroupMode::None => pipeline,
            GroupMode::Extension => pipeline.group_by(filer_core::pipeline::GroupBy::Extension),
            GroupMode::Date => pipeline.group_by(filer_core::pipeline::GroupBy::Date),
            GroupMode::Size => pipeline.group_by(filer_core::pipeline::GroupBy::Size),
            GroupMode::FirstLetter => pipeline.group_by(filer_core::pipeline::GroupBy::FirstLetter),
        };
        pipeline
    }

    fn rescan_with_current_pipeline(&self) {
        let pipeline = self.current_pipeline();
        debug!(
            session = %self.session,
            group = ?self.group_mode,
            sort = ?self.sort_field,
            dir = ?self.sort_dir,
            "ui -> core scan with current pipeline"
        );
        let _ = self.core.send(Command::SetPipeline {
            session: self.session,
            config: pipeline.clone(),
        });
        let _ = self.core.send(Command::Scan {
            path: self.state.current_path.clone(),
            session: self.session,
            pipeline,
            request: RequestId::new(),
        });
    }
}

fn map_pointer_event(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::PointerMoved(position))
        }
        _ => None,
    }
}
