pub mod breadcrumb;
pub mod context_menu;
pub mod file_list;
pub mod operations;
pub mod preview;
pub mod search;
pub mod sidebar;
pub mod status_bar;

use filer_core::FileNode;
use iced::widget::{column, container, row, rule};
use iced::{Element, Length};

use crate::message::{Message, SortDir, SortField};
use crate::state::AppState;

/// Build the full application layout.
pub fn view(state: &AppState, sort_field: &SortField, sort_dir: &SortDir) -> Element<'static, Message> {
    let all_nodes: Vec<FileNode> = state.visible_nodes();

    // ── Left: sidebar ────────────────────────────────────────────────
    let sidebar = sidebar::view(state);

    // ── Centre: breadcrumb + search + file list + status ─────────────
    let breadcrumb = breadcrumb::view(&state.current_path);
    let search_bar = search::view(&state.search_query);
    let file_list = file_list::view(state);
    let status = status_bar::view(state, sort_field, sort_dir);

    let rename_overlay: Option<Element<Message>> = state.rename_state.as_ref().map(|r| {
        let input = iced::widget::text_input("New name...", &r.current_name)
            .on_input(Message::RenameInput)
            .on_submit(Message::RenameCommit)
            .size(13)
            .padding(6);
        container(input)
            .padding(8)
            .into()
    });

    let create_folder_overlay: Option<Element<Message>> = state.create_folder_state.as_ref().map(|c| {
        let input = iced::widget::text_input("Folder name...", &c.name)
            .on_input(Message::CreateFolderInput)
            .on_submit(Message::CreateFolderCommit)
            .size(13)
            .padding(6);
        container(input).padding(8).into()
    });

    let mut centre_col: Vec<Element<Message>> = vec![
        row![breadcrumb, search_bar].spacing(8).padding([4, 8]).into(),
        rule::horizontal(1).into(),
    ];

    if let Some(overlay) = rename_overlay {
        centre_col.push(overlay);
    }
    if let Some(overlay) = create_folder_overlay {
        centre_col.push(overlay);
    }

    centre_col.push(file_list);
    centre_col.push(rule::horizontal(1).into());
    centre_col.push(status);

    let centre = column(centre_col).width(Length::Fill);

    // ── Right: preview panel ─────────────────────────────────────────
    let body = if state.preview_visible {
        let prev = preview::view(state, &all_nodes);
        row![sidebar, centre, prev].spacing(0).into()
    } else {
        row![sidebar, centre].spacing(0).into()
    };

    // ── Context menu overlay ─────────────────────────────────────────
    if let Some(ctx) = &state.context_menu {
        let menu = context_menu::view(state, ctx);
        // Use iced::widget::stack for absolute overlay.
        iced::widget::stack![body, menu].into()
    } else if let Some(op) = &state.operation_progress {
        let prog = operations::view(op);
        iced::widget::stack![body, prog].into()
    } else {
        body
    }
}
