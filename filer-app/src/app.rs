use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use filer_core::model::node::NodeId;
use filer_core::model::session::SessionId;
use filer_core::{Command, FilerCore, PreviewData};
use iced::futures::Stream;
use iced::keyboard::{Key, Modifiers};
use iced::{Element, Subscription, Task, Theme};

use crate::config::Config;
use crate::message::{Message, SortDir, SortField};
use crate::state::{
    AppState, ClipboardState, ContextMenuState, CreateFolderState, OperationProgressState,
    RenameState, SelectMode,
};
use crate::views;

// ─── Subscription bridge ────────────────────────────────────────────────────

/// Newtype wrapping a flume receiver so it can satisfy `Hash` (required by
/// `Subscription::run_with`). Hash is fixed — there is always exactly one
/// core subscription and it must never restart.
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
        Key::Character(c) if c.as_str() == "/" => Some(Message::SearchInput(String::new())),
        _ => None,
    }
}

// ─── App ────────────────────────────────────────────────────────────────────

pub struct App {
    core: Arc<FilerCore>,
    session: SessionId,
    state: AppState,
    sort_field: SortField,
    sort_dir: SortDir,
    config: Config,
}

impl App {
    pub fn new(core: Arc<FilerCore>) -> (Self, Task<Message>) {
        let config = Config::load();

        // Handshake registers a session in the core SessionManager and
        // returns SessionCreated(id). We use that id for all subsequent
        // commands — do NOT navigate here; navigate inside handle_core_event.
        tracing::info!("App::new — sending Handshake");
        let _ = core.send(Command::Handshake);

        let state = AppState::new(config.preview_visible, config.bookmarks.clone());

        let app = Self {
            core,
            session: SessionId::DEFAULT, // replaced on SessionCreated
            state,
            sort_field: SortField::Name,
            sort_dir: SortDir::Asc,
            config,
        };
        (app, Task::none())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard = iced::keyboard::listen()
            .filter_map(map_keyboard);
        Subscription::batch([
            Subscription::run_with(CoreRx(self.core.event_receiver()), core_event_stream),
            keyboard,
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CoreEvent(event) => self.handle_core_event(event),

            Message::Navigate(path) => {
                tracing::info!(path = ?path, "Navigate message received");
                self.state.search_active = false;
                self.state.search_results = None;
                self.state.search_query.clear();
                self.state.selection.clear();
                self.state.preview_data = None;
                let _ = self.core.send(Command::Navigate(path, self.session));
                Task::none()
            }
            Message::NavigateBack => {
                let _ = self.core.send(Command::NavigateBack(self.session));
                Task::none()
            }
            Message::NavigateForward => Task::none(),
            Message::NavigateUp => {
                let _ = self.core.send(Command::NavigateUp(self.session));
                Task::none()
            }
            Message::Refresh => {
                let _ = self.core.send(Command::Refresh(self.session));
                Task::none()
            }

            Message::SelectNode(id, mode) => {
                let all_ids = self.state.visible_ids();
                match mode {
                    SelectMode::Single => self.state.selection.select_one(id),
                    SelectMode::Toggle => self.state.selection.toggle(id),
                    SelectMode::Range => self.state.selection.range_to(id, &all_ids),
                }
                if self.state.selection.len() == 1 {
                    self.state.preview_node = Some(id);
                    self.state.preview_data = None;
                    // Only load previews for files; directories have no meaningful content preview.
                    let is_file = self.state.visible_nodes()
                        .iter()
                        .find(|n| n.id == id)
                        .map(|n| matches!(n.kind, filer_core::model::node::NodeKind::File { .. }))
                        .unwrap_or(false);
                    if is_file {
                        let _ = self.core.send(Command::LoadPreview {
                            id,
                            options: None,
                            session: self.session,
                        });
                    }
                }
                Task::none()
            }
            Message::SelectAll => {
                let ids = self.state.visible_ids();
                self.state.selection.select_all(&ids);
                Task::none()
            }

            Message::OpenNode(id) => {
                if let Some(path) = self.resolve(id) {
                    if path.is_dir() {
                        return self.update(Message::Navigate(path));
                    }
                }
                Task::none()
            }

            Message::SearchInput(query) => {
                self.state.search_bar_open = true;
                self.state.search_query = query.clone();
                self.state.search_pending = Some(query);
                Task::perform(
                    async { tokio::time::sleep(Duration::from_millis(150)).await },
                    |_| Message::Tick,
                )
            }
            Message::Tick => {
                if let Some(query) = self.state.search_pending.take() {
                    if query.is_empty() {
                        self.state.search_active = false;
                        self.state.search_results = None;
                    } else {
                        self.state.search_active = true;
                        let root = self.core.registry().register(self.state.current_path.clone());
                        let _ = self.core.send(Command::Search {
                            query,
                            root,
                            session: self.session,
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
                self.state.search_bar_open = false;
                Task::none()
            }

            Message::ShowContextMenu(node, point) => {
                if !self.state.selection.contains(node) {
                    self.state.selection.select_one(node);
                }
                self.state.context_menu = Some(ContextMenuState { node, position: point });
                Task::none()
            }
            Message::HideContextMenu => {
                self.state.context_menu = None;
                self.state.rename_state = None;
                self.state.create_folder_state = None;
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
                            });
                            self.state.clipboard = None;
                        } else {
                            let _ = self.core.send(Command::Copy {
                                sources: cb.nodes,
                                destination: dest,
                                session: self.session,
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
                self.state.rename_state = Some(RenameState { node: id, current_name: name });
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
                    });
                }
                Task::none()
            }
            Message::RenameCancel => {
                self.state.rename_state = None;
                Task::none()
            }

            Message::CreateFolderStart => {
                self.state.create_folder_state = Some(CreateFolderState { name: String::new() });
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

            Message::TogglePreview => {
                self.state.preview_visible = !self.state.preview_visible;
                self.config.preview_visible = self.state.preview_visible;
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
                self.sort_field = field;
                self.sort_dir = dir;
                let _ = self.core.send(Command::Navigate(
                    self.state.current_path.clone(),
                    self.session,
                ));
                Task::none()
            }

            Message::OperationProgress(kind, done, total) => {
                self.state.operation_progress = Some(OperationProgressState {
                    label: format!("{kind:?}"),
                    done,
                    total,
                });
                Task::none()
            }
        }
    }

    fn handle_core_event(&mut self, event: filer_core::Event) -> Task<Message> {
        match event {
            filer_core::Event::DirectoryLoaded { path, groups, .. } => {
                tracing::info!(path = ?path, nodes = groups.total_count, "DirectoryLoaded");
                self.state.current_path = path;
                self.state.groups = groups;
                self.state.selection.clear();
                self.state.operation_progress = None;
            }
            filer_core::Event::SearchResults { matches, complete, .. } => {
                let existing = self.state.search_results.get_or_insert_with(Vec::new);
                existing.extend(matches);
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
                let _ = self.core.send(Command::Refresh(self.session));
            }
            filer_core::Event::OperationProgress {
                operation,
                items_done,
                total_items,
                ..
            } => {
                return self.update(Message::OperationProgress(
                    operation,
                    items_done,
                    total_items,
                ));
            }
            filer_core::Event::SessionCreated(id) => {
                tracing::info!(?id, "session created — navigating home");
                self.session = id;
                let home = self.state.current_path.clone();
                let _ = self.core.send(Command::Navigate(home, id));
            }
            filer_core::Event::CurrentNavigateState { state, .. } => {
                self.state.nav = Some(state);
            }
            filer_core::Event::Error { message, recoverable, .. } => {
                tracing::warn!(recoverable, "core error: {message}");
            }
            filer_core::Event::FsChanged { .. } => {
                let _ = self.core.send(Command::Refresh(self.session));
            }
            _ => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        views::view(&self.state, &self.sort_field, &self.sort_dir)
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn resolve(&self, id: NodeId) -> Option<PathBuf> {
        self.core.registry().resolve(id)
    }

    fn current_node_id(&self) -> Option<NodeId> {
        Some(self.core.registry().register(self.state.current_path.clone()))
    }
}
