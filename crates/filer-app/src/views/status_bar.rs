use iced::widget::{container, row, text};
use iced::{Element, Length};

use crate::format::size_human;
use crate::message::{Message, SortDir, SortField};
use crate::state::AppState;
use crate::views::theme::panel_alt_surface;

/// Build the status bar at the bottom of the file list.
pub fn view(
    state: &AppState,
    sort_field: &SortField,
    sort_dir: &SortDir,
) -> Element<'static, Message> {
    let total = state.groups.total_count;
    let sel_count = state.selection.len();
    let sel_size = state.selected_size();

    let item_label = if total == 1 {
        "1 item".to_string()
    } else {
        format!("{total} items")
    };

    let sel_label = if sel_count == 0 {
        String::new()
    } else {
        format!(
            "  ·  {sel_count} selected ({sel_size_str})",
            sel_size_str = size_human(sel_size)
        )
    };

    let sort_label = format!(
        "Sort: {} {}",
        match sort_field {
            SortField::Name => "Name",
            SortField::Size => "Size",
            SortField::Modified => "Modified",
            SortField::Kind => "Type",
        },
        match sort_dir {
            SortDir::Asc => "A→Z",
            SortDir::Desc => "Z→A",
        }
    );

    let search_label = if state.search_query.is_empty() {
        "Browse".to_string()
    } else if state.search_active {
        "Searching...".to_string()
    } else {
        format!("Search: {} match(es)", state.search_result_count)
    };
    let preview_label = if state.preview_visible {
        "Preview: On"
    } else {
        "Preview: Off"
    };

    container(
        row![
            text(format!("{item_label}{sel_label}"))
                .size(11)
                .width(Length::Fill),
            text(sort_label).size(11),
            text(" · ").size(11),
            text(search_label).size(11),
            text(" · ").size(11),
            text(preview_label).size(11),
        ]
        .spacing(4)
        .padding([4, 8]),
    )
    .style(panel_alt_surface)
    .into()
}
