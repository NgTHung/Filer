use iced::widget::{button, column, container, text};
use iced::{Element, Length};

use crate::icon;
use crate::message::Message;
use crate::state::{AppState, ContextMenuState, PanelTab};
use crate::views::theme::palette;

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
        menu_item(icon::file(), "Open", Message::OpenSelected, false),
        menu_item(
            icon::info(),
            "Details",
            Message::SetPanelTab(PanelTab::Details),
            false,
        ),
        menu_item(icon::copy(), &copy_label, Message::CopySelected, false),
        menu_item(icon::cut(), &cut_label, Message::CutSelected, false),
    ];

    if has_clipboard {
        items.push(menu_item(icon::paste(), "Paste", Message::Paste, false));
    }

    items.push(menu_item(
        icon::trash(),
        &delete_label,
        Message::DeleteSelected { trash: true },
        false,
    ));
    items.push(menu_item(
        icon::trash(),
        "Delete Permanently",
        Message::DeleteSelected { trash: false },
        true,
    ));

    if sel_count == 1 {
        items.push(menu_item(
            icon::pencil(),
            "Rename",
            Message::RenameStart(ctx.node),
            false,
        ));
    }

    items.push(menu_item(
        icon::folder_plus(),
        "New Folder",
        Message::CreateFolderStart,
        false,
    ));

    container(
        column(items)
            .spacing(2)
            .padding(4)
            .width(Length::Fixed(180.0)),
    )
    .style(menu_style)
    .into()
}

fn menu_item(
    glyph: iced::widget::Text<'static>,
    label: &str,
    msg: Message,
    danger: bool,
) -> Element<'static, Message> {
    button(iced::widget::row![glyph.size(13), text(label.to_owned()).size(13)].spacing(8))
        .on_press(msg)
        .width(Length::Fill)
        .padding([5, 10])
        .style(move |theme, status| item_style(theme, status, danger))
        .into()
}

fn menu_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let c = palette(theme);
    iced::widget::container::Style {
        background: Some(iced::Background::Color(c.surface)),
        border: iced::Border {
            color: c.border,
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

fn item_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
    danger: bool,
) -> iced::widget::button::Style {
    let c = palette(theme);
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(iced::Background::Color(c.surface_alt)),
            text_color: if danger { c.danger } else { c.text },
            border: iced::Border {
                color: c.border,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color: if danger { c.danger } else { c.text },
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
    }
}
