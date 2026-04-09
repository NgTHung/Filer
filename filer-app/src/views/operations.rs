use iced::widget::{column, container, progress_bar, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::OperationProgressState;

/// Build the operation-progress overlay.
pub fn view(op: &OperationProgressState) -> Element<'static, Message> {
    let fraction = if op.total > 0 {
        op.done as f32 / op.total as f32
    } else {
        0.0
    };

    let label = format!("{} ({}/{})", op.label, op.done, op.total);

    container(
        column![
            text(label).size(13),
            progress_bar(0.0..=1.0, fraction).length(Length::Fixed(280.0)),
        ]
        .spacing(8)
        .padding(16),
    )
    .style(overlay_style)
    .into()
}

fn overlay_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();
    iced::widget::container::Style {
        background: Some(iced::Background::Color(palette.background.base.color)),
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        text_color: None,
        snap: false,
    }
}
