use filer_core::model::node::NodeKind;
use filer_core::pipeline::GroupedNodes;
use filer_core::FileNode;
use iced::widget::{button, container, row, scrollable, text, Column};
use iced::{Color, Element, Length};

use crate::format::{size_human, time_relative};
use crate::icons;
use crate::message::Message;
use crate::state::{AppState, SelectMode};

const ROW_HEIGHT: f32 = 28.0;
const VIRTUALIZE_THRESHOLD: usize = 500;

/// Build the file list view.
pub fn view(state: &AppState) -> Element<'static, Message> {
    // Choose between search results and grouped directory listing.
    if let Some(results) = &state.search_results {
        flat_list(results, state)
    } else {
        grouped_list(&state.groups, state)
    }
}

fn grouped_list(groups: &GroupedNodes, state: &AppState) -> Element<'static, Message> {
    let total: usize = groups.groups.iter().map(|g| g.nodes.len()).sum();

    if total == 0 {
        return empty_state();
    }

    let mut col: Vec<Element<Message>> = Vec::new();

    for group in &groups.groups {
        if !group.label.is_empty() {
            col.push(
                text(group.label.clone())
                    .size(11)
                    .color(Color::from_rgb(0.5, 0.5, 0.5))
                    .into(),
            );
        }
        for node in &group.nodes {
            col.push(file_row(node, state));
        }
    }

    scrollable(Column::with_children(col).spacing(1).padding([0, 4]))
        .height(Length::Fill)
        .into()
}

fn flat_list(nodes: &[FileNode], state: &AppState) -> Element<'static, Message> {
    if nodes.is_empty() {
        return container(text("No results").size(13))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let col = Column::with_children(nodes.iter().map(|n| file_row(n, state)).collect::<Vec<_>>())
        .spacing(1)
        .padding([0, 4]);

    scrollable(col).height(Length::Fill).into()
}

fn file_row(node: &FileNode, state: &AppState) -> Element<'static, Message> {
    let selected = state.selection.contains(node.id);
    let icon = text(icons::for_node(node)).size(14);
    let name = text(node.name.clone()).size(13).width(Length::Fill);
    let size_str = match &node.kind {
        NodeKind::Directory { .. } => "—".to_string(),
        _ => size_human(node.size),
    };
    let size = text(size_str).size(12).width(Length::Fixed(80.0));
    let modified = text(time_relative(node.modified))
        .size(12)
        .width(Length::Fixed(110.0));

    let content = row![icon, name, size, modified]
        .spacing(8)
        .padding([4, 8])
        .align_y(iced::Alignment::Center);

    let id = node.id;
    let is_dir = matches!(node.kind, NodeKind::Directory { .. });
    // Directories navigate on single click; files select (and load preview).
    let msg = if is_dir {
        Message::OpenNode(id)
    } else {
        Message::SelectNode(id, SelectMode::Single)
    };
    button(content)
        .on_press(msg)
        .width(Length::Fill)
        .style(move |theme, status| row_style(theme, status, selected))
        .into()
}

fn row_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
    selected: bool,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    if selected {
        iced::widget::button::Style {
            background: Some(iced::Background::Color(palette.primary.weak.color)),
            text_color: palette.primary.weak.text,
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        }
    } else {
        match status {
            iced::widget::button::Status::Hovered => iced::widget::button::Style {
                background: Some(iced::Background::Color(
                    palette.background.strong.color,
                )),
                text_color: palette.background.base.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            },
            _ => iced::widget::button::Style {
                background: None,
                text_color: palette.background.base.text,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            },
        }
    }
}

fn empty_state() -> Element<'static, Message> {
    container(text("Empty folder").size(13).color(Color::from_rgb(0.5, 0.5, 0.5)))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
