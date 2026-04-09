use iced::widget::{button, row, text_input};
use iced::{Element, Length};

use crate::message::Message;

/// Build the search bar widget.
pub fn view(query: &str) -> Element<'static, Message> {
    let input = text_input("Search...", query)
        .on_input(Message::SearchInput)
        .on_submit(Message::SearchCommit)
        .width(Length::Fill)
        .size(13)
        .padding(6);

    let clear_btn = button(iced::widget::text("✕").size(13))
        .on_press(Message::SearchClear)
        .padding([4, 8]);

    row![input, clear_btn].spacing(4).into()
}
