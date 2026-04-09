use iced::widget::{row, text};
use iced::{Element, Length};

use crate::format::size_human;
use crate::message::{Message, SortDir, SortField};
use crate::state::AppState;

/// Build the status bar at the bottom of the file list.
pub fn view(state: &AppState, sort: &SortField, dir: &SortDir) -> Element<'static, Message> {
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
        format!("  ·  {sel_count} selected ({sel_size_str})", sel_size_str = size_human(sel_size))
    };

    let sort_label = format!(
        "  ·  Sort: {} {}",
        match sort {
            SortField::Name => "Name",
            SortField::Size => "Size",
            SortField::Modified => "Modified",
            SortField::Kind => "Kind",
        },
        match dir {
            SortDir::Asc => "↑",
            SortDir::Desc => "↓",
        }
    );

    row![
        text(format!("{item_label}{sel_label}{sort_label}"))
            .size(12)
            .width(Length::Fill),
    ]
    .padding([4, 8])
    .into()
}
