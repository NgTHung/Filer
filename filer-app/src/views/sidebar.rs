use std::path::PathBuf;

use iced::widget::{button, text, Column};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::AppState;

/// Build the sidebar: places + user bookmarks.
pub fn view(state: &AppState) -> Element<'static, Message> {
    let mut col: Vec<Element<Message>> = Vec::new();

    // ── Places ───────────────────────────────────────────────────────
    col.push(
        text("Places")
            .size(11)
            .color(iced::Color::from_rgb(0.5, 0.5, 0.5))
            .into(),
    );
    for (label, path) in &state.places {
        col.push(place_button(label, path.clone(), &state.current_path));
    }

    // ── Bookmarks ────────────────────────────────────────────────────
    if !state.bookmarks.is_empty() {
        col.push(
            text("Bookmarks")
                .size(11)
                .color(iced::Color::from_rgb(0.5, 0.5, 0.5))
                .into(),
        );
        for path in &state.bookmarks {
            let label = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            col.push(place_button(&label, path.clone(), &state.current_path));
        }
    }

    Column::with_children(col)
        .spacing(2)
        .padding(8)
        .width(Length::Fixed(200.0))
        .into()
}

fn place_button(
    label: &str,
    path: PathBuf,
    current: &PathBuf,
) -> Element<'static, Message> {
    let is_active = path == *current;
    let style = if is_active {
        iced::widget::button::primary
    } else {
        iced::widget::button::secondary
    };
    button(text(label.to_owned()).size(13))
        .on_press(Message::Navigate(path))
        .style(style)
        .width(Length::Fill)
        .padding([4, 8])
        .into()
}
