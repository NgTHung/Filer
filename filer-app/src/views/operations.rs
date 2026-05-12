use iced::widget::{column, container, progress_bar, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::OperationProgressState;
use crate::views::theme::panel_alt_surface;

/// Build the operation-progress tray.
pub fn view(op: &OperationProgressState) -> Element<'static, Message> {
    let _ = op.operation_id;
    let fraction = if op.total > 0 {
        op.done as f32 / op.total as f32
    } else {
        0.0
    };

    let label = format!("{} ({}/{})", op.label, op.done, op.total);

    container(
        column![
            text(label).size(11),
            progress_bar(0.0..=1.0, fraction).length(Length::Fill),
        ]
        .spacing(6)
        .padding([6, 10]),
    )
    .style(tray_style)
    .width(Length::Fill)
    .into()
}

fn tray_style(theme: &iced::Theme) -> iced::widget::container::Style {
    panel_alt_surface(theme)
}
