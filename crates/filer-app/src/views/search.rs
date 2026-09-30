use iced::widget::{button, row, text, text_input};
use iced::{Element, Length};

use crate::icon;
use crate::message::Message;
use crate::views::theme::top_button;

/// Build the search bar widget.
pub fn view(query: &str, search_active: bool, result_count: usize) -> Element<'static, Message> {
    let input = text_input("Search...", query)
        .on_input(Message::SearchInput)
        .on_submit(Message::SearchCommit)
        .width(Length::Fixed(260.0))
        .size(12)
        .padding(6);

    let status_label = if query.is_empty() {
        "Ready".to_string()
    } else if search_active {
        "Searching...".to_string()
    } else {
        format!("{result_count} match(es)")
    };

    let clear_btn = button(iced::widget::text("Clear").size(12))
        .on_press(Message::SearchClear)
        .style(|theme, status| top_button(theme, status, false))
        .padding([4, 8]);

    row![
        icon::search().size(14),
        input,
        text(status_label).size(11).width(Length::Fixed(100.0)),
        clear_btn
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}
