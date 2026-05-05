use std::path::PathBuf;

use iced::widget::{Column, button, container, row, rule, scrollable, text};
use iced::{Element, Length};

use crate::config::ThemeMode;
use crate::message::Message;
use crate::state::AppState;
use crate::views::theme::{palette, top_button};

pub fn view(state: &AppState, theme_mode: &ThemeMode) -> Element<'static, Message> {
    let mut sections: Vec<Element<Message>> = vec![section_title("Quick Access")];
    for (label, path) in &state.places {
        sections.push(path_button(label, path.clone(), &state.current_path));
    }

    sections.push(rule::horizontal(1).into());
    sections.push(section_title("Bookmarks"));
    if state.bookmarks.is_empty() {
        sections.push(muted("No bookmarks yet"));
    } else {
        for path in &state.bookmarks {
            sections.push(bookmark_row(path, &state.current_path));
        }
    }

    sections.push(rule::horizontal(1).into());
    sections.push(section_title("Recent"));
    if state.recent_paths.is_empty() {
        sections.push(muted("No recent folders"));
    } else {
        for path in &state.recent_paths {
            sections.push(path_button(
                &path_label(path),
                path.clone(),
                &state.current_path,
            ));
        }
    }

    sections.push(rule::horizontal(1).into());
    sections.push(section_title("Theme"));
    sections.push(theme_buttons(theme_mode));

    container(
        scrollable(Column::with_children(sections).spacing(4).padding(10)).height(Length::Fill),
    )
    .width(Length::Fixed(230.0))
    .style(sidebar_style)
    .into()
}

fn section_title(title: &str) -> Element<'static, Message> {
    text(title.to_owned())
        .size(11)
        .color(iced::Color::from_rgb(0.42, 0.52, 0.62))
        .into()
}

fn muted(label: &str) -> Element<'static, Message> {
    text(label.to_owned())
        .size(11)
        .color(iced::Color::from_rgb(0.45, 0.45, 0.45))
        .into()
}

fn path_button(label: &str, path: PathBuf, current: &PathBuf) -> Element<'static, Message> {
    let is_active = path == *current;
    let active = is_active;

    button(text(label.to_owned()).size(12))
        .on_press(Message::Navigate(path))
        .style(move |theme, status| top_button(theme, status, active))
        .width(Length::Fill)
        .padding([4, 8])
        .into()
}

fn bookmark_row(path: &PathBuf, current: &PathBuf) -> Element<'static, Message> {
    let target = path.clone();
    let remove_target = path.clone();
    let is_active = target == *current;
    let active = is_active;

    row![
        button(text(path_label(path)).size(12))
            .on_press(Message::Navigate(target))
            .style(move |theme, status| top_button(theme, status, active))
            .width(Length::Fill),
        button(text("×").size(12))
            .on_press(Message::RemoveBookmark(remove_target))
            .style(|theme, status| top_button(theme, status, false))
            .padding([2, 6]),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .into()
}

fn theme_buttons(active: &ThemeMode) -> Element<'static, Message> {
    let auto = theme_btn("Auto", ThemeMode::Auto, active);
    let light = theme_btn("Light", ThemeMode::Light, active);
    let dark = theme_btn("Dark", ThemeMode::Dark, active);
    row![auto, light, dark].spacing(4).into()
}

fn theme_btn(
    label: &'static str,
    mode: ThemeMode,
    active: &ThemeMode,
) -> Element<'static, Message> {
    let is_active = active == &mode;
    button(text(label).size(12))
        .on_press(Message::SetThemeMode(mode))
        .style(move |theme, status| top_button(theme, status, is_active))
        .padding([4, 8])
        .into()
}

fn path_label(path: &PathBuf) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn sidebar_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let c = palette(theme);
    iced::widget::container::Style {
        background: Some(iced::Background::Color(c.surface_alt)),
        border: iced::Border {
            color: c.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: None,
        snap: false,
    }
}
