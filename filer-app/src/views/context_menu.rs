use iced::widget::{button, column, container, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::{AppState, ContextMenuState};

/// Build the context menu overlay at `ctx.position`.
///
/// The caller is responsible for placing this at the right position using
/// `iced::widget::stack` or similar absolute-positioning primitives.
pub fn view(state: &AppState, ctx: &ContextMenuState) -> Element<'static, Message> {
    let has_clipboard = state.clipboard.is_some();
    let multi = state.selection.len() > 1;
    let sel_count = state.selection.len();

    let copy_label = if multi {
        format!("Copy {sel_count} items")
    } else {
        "Copy".to_string()
    };
    let delete_label = if multi {
        format!("Move {sel_count} to Trash")
    } else {
        "Move to Trash".to_string()
    };

    let cut_label = if multi {
        format!("Cut {sel_count} items")
    } else {
        "Cut".to_string()
    };

    let mut items: Vec<Element<Message>> = vec![
        menu_item(&copy_label, Message::CopySelected),
        menu_item(&cut_label, Message::CutSelected),
    ];

    if has_clipboard {
        items.push(menu_item("Paste", Message::Paste));
    }

    items.push(menu_item(&delete_label, Message::DeleteSelected { trash: true }));
    items.push(menu_item("Delete Permanently", Message::DeleteSelected { trash: false }));

    if sel_count == 1 {
        items.push(menu_item("Rename", Message::RenameStart(ctx.node)));
    }

    items.push(menu_item("New Folder", Message::CreateFolderStart));

    container(
        column(items).spacing(2).padding(4).width(Length::Fixed(180.0)),
    )
    .style(menu_style)
    .into()
}

fn menu_item(label: &str, msg: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(13))
        .on_press(msg)
        .width(Length::Fill)
        .padding([5, 10])
        .style(iced::widget::button::text)
        .into()
}

fn menu_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();
    iced::widget::container::Style {
        background: Some(iced::Background::Color(palette.background.base.color)),
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector::new(2.0, 2.0),
            blur_radius: 6.0,
        },
        text_color: None,
        snap: false,
    }
}
