pub mod breadcrumb;
pub mod context_menu;
pub mod file_list;
pub mod operations;
pub mod preview;
pub mod search;
pub mod sidebar;
pub mod status_bar;
pub mod theme;

use iced::widget::{button, column, container, row, rule, text, text_input};
use iced::{Color, Element, Length};

use crate::config::ThemeMode;
use crate::icon;
use crate::message::{Message, SortDir, SortField};
use crate::state::AppState;
use theme::{panel_alt_surface, panel_surface, top_button};

pub fn view<'a>(
    state: &'a AppState,
    sort_field: &'a SortField,
    sort_dir: &'a SortDir,
    theme_mode: &'a ThemeMode,
) -> Element<'a, Message> {
    let sidebar = sidebar::view(state, theme_mode);
    let command = command_bar(state);
    let file_list = file_list::view(state, sort_field, sort_dir);
    let status = status_bar::view(state, sort_field, sort_dir);

    let mut center_parts: Vec<Element<Message>> = vec![command, rule::horizontal(1).into()];

    if let Some(msg) = &state.error_message {
        center_parts.push(error_banner(msg));
    }

    if let Some(rename) = &state.rename_state {
        center_parts.push(inline_input(
            "Rename selected item",
            "New name...",
            &rename.current_name,
            Message::RenameInput,
            Message::RenameCommit,
            Message::RenameCancel,
        ));
    }

    if let Some(create) = &state.create_folder_state {
        center_parts.push(inline_input(
            "Create folder",
            "Folder name...",
            &create.name,
            Message::CreateFolderInput,
            Message::CreateFolderCommit,
            Message::CreateFolderCancel,
        ));
    }

    center_parts.push(file_list);

    if let Some(progress) = &state.operation_progress {
        center_parts.push(operations::view(progress));
    }

    center_parts.push(rule::horizontal(1).into());
    center_parts.push(status);

    let center = column(center_parts).width(Length::Fill);

    let body: Element<Message> = if state.preview_visible {
        row![sidebar, center, preview::view(state)]
            .spacing(0)
            .into()
    } else {
        row![sidebar, center].spacing(0).into()
    };
    let body: Element<Message> = iced::widget::mouse_area(body)
        .on_press(Message::HideContextMenuOnly)
        .into();

    if let Some(ctx) = &state.context_menu {
        iced::widget::stack![body, context_menu_overlay(state, ctx)].into()
    } else {
        body
    }
}

fn context_menu_overlay<'a>(
    state: &'a AppState,
    ctx: &'a crate::state::ContextMenuState,
) -> Element<'a, Message> {
    let x = ctx.position.x.clamp(4.0, 1300.0);
    let y = ctx.position.y.clamp(4.0, 700.0);
    container(column![
        iced::widget::Space::new().height(Length::Fixed(y)),
        row![
            iced::widget::Space::new().width(Length::Fixed(x)),
            context_menu::view(state, ctx)
        ]
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn command_bar<'a>(state: &'a AppState) -> Element<'a, Message> {
    let can_back = state.nav.as_ref().is_some_and(|n| n.can_back);
    let can_forward = state.nav.as_ref().is_some_and(|n| n.can_forward);
    let has_clipboard = state.clipboard.is_some();

    let back = nav_btn(icon::back().size(14), Message::NavigateBack, can_back);
    let forward = nav_btn(
        icon::forward().size(14),
        Message::NavigateForward,
        can_forward,
    );
    let up = nav_btn(icon::up().size(14), Message::NavigateUp, true);
    let refresh = nav_btn(icon::refresh().size(14), Message::Refresh, true);

    let bookmark = button(row![icon::bookmark().size(14), text("Bookmark").size(12)].spacing(6))
        .on_press(Message::AddBookmark)
        .style(|theme, status| top_button(theme, status, false))
        .padding([4, 10]);

    let paste = if has_clipboard {
        button(row![icon::paste().size(14), text("Paste").size(12)].spacing(6))
            .on_press(Message::Paste)
            .style(|theme, status| top_button(theme, status, false))
            .padding([4, 10])
    } else {
        button(row![icon::paste().size(14), text("Paste").size(12)].spacing(6))
            .style(|theme, status| top_button(theme, status, false))
            .padding([4, 10])
    };

    let new_folder =
        button(row![icon::folder_plus().size(14), text("New Folder").size(12)].spacing(6))
            .on_press(Message::CreateFolderStart)
            .style(|theme, status| top_button(theme, status, false))
            .padding([4, 10]);

    let preview_label = if state.preview_visible {
        "Hide Panel"
    } else {
        "Show Panel"
    };
    let panel_toggle =
        button(row![icon::panel_right().size(14), text(preview_label).size(12)].spacing(6))
            .on_press(Message::TogglePreview)
            .style(|theme, status| top_button(theme, status, state.preview_visible))
            .padding([4, 10]);

    let top = row![
        back,
        forward,
        up,
        refresh,
        bookmark,
        paste,
        new_folder,
        panel_toggle
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let path = text_input("Path", &state.address_input)
        .on_input(Message::AddressInputChanged)
        .on_submit(Message::AddressSubmit)
        .padding(6)
        .size(12)
        .width(Length::Fill);
    let go = button(text("Go").size(12))
        .on_press(Message::AddressSubmit)
        .style(|theme, status| top_button(theme, status, false))
        .padding([4, 8]);
    let search = search::view(
        &state.search_query,
        state.search_active,
        state.search_result_count,
    );

    container(
        column![top, row![path, go, search].spacing(10)]
            .spacing(8)
            .padding([8, 10]),
    )
    .style(panel_alt_surface)
    .into()
}

fn nav_btn(
    label: iced::widget::Text<'static>,
    msg: Message,
    enabled: bool,
) -> Element<'static, Message> {
    let base = button(label)
        .style(|theme, status| top_button(theme, status, false))
        .padding([4, 8])
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(30.0));

    if enabled {
        base.on_press(msg).into()
    } else {
        base.into()
    }
}

fn inline_input(
    title: &'static str,
    placeholder: &'static str,
    value: &str,
    on_input: fn(String) -> Message,
    on_submit: Message,
    on_cancel: Message,
) -> Element<'static, Message> {
    container(
        row![
            text(title).size(12).width(Length::Fixed(140.0)),
            iced::widget::text_input(placeholder, value)
                .on_input(on_input)
                .on_submit(on_submit)
                .padding(6)
                .size(12)
                .width(Length::Fill),
            button(text("Cancel").size(12))
                .on_press(on_cancel)
                .style(iced::widget::button::text)
                .padding([2, 6]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .style(panel_surface)
    .padding([4, 10])
    .into()
}

fn error_banner(message: &str) -> Element<'static, Message> {
    container(
        row![
            text(format!("Warning: {message}"))
                .size(12)
                .width(Length::Fill)
                .color(Color::from_rgb(1.0, 0.85, 0.3)),
            button(text("Dismiss").size(12))
                .on_press(Message::DismissError)
                .style(iced::widget::button::text)
                .padding([2, 6]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .style(error_banner_style)
    .padding([4, 10])
    .into()
}

fn error_banner_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.55, 0.20, 0.0, 0.42,
        ))),
        border: iced::Border {
            color: iced::Color::from_rgb(0.8, 0.45, 0.1),
            width: 1.0,
            radius: 6.0.into(),
        },
        text_color: None,
        shadow: iced::Shadow::default(),
        snap: false,
    }
}
