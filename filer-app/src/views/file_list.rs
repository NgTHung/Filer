use filer_core::FileNode;
use filer_core::model::node::NodeKind;
use filer_core::pipeline::GroupedNodes;
use iced::widget::{Column, button, container, mouse_area, row, scrollable, text};
use iced::{Color, Element, Length};

use crate::format::{size_human, time_relative};
use crate::icons;
use crate::message::{Message, SortDir, SortField};
use crate::state::AppState;
use crate::views::theme::palette;

pub fn view(
    state: &AppState,
    sort_field: &SortField,
    sort_dir: &SortDir,
) -> Element<'static, Message> {
    let rows = if let Some(results) = &state.search_results {
        flat_list(results, state)
    } else {
        grouped_list(&state.groups, state)
    };

    Column::new()
        .push(header_row(sort_field, sort_dir))
        .push(iced::widget::rule::horizontal(1))
        .push(rows)
        .height(Length::Fill)
        .into()
}

fn header_row(sort_field: &SortField, sort_dir: &SortDir) -> Element<'static, Message> {
    let indicator = |field: &SortField| -> &'static str {
        if field == sort_field {
            match sort_dir {
                SortDir::Asc => " ↑",
                SortDir::Desc => " ↓",
            }
        } else {
            ""
        }
    };

    let next_dir = |field: &SortField| -> SortDir {
        if field == sort_field {
            match sort_dir {
                SortDir::Asc => SortDir::Desc,
                SortDir::Desc => SortDir::Asc,
            }
        } else {
            SortDir::Asc
        }
    };

    let icon_spacer = text("").width(Length::Fixed(24.0));
    let name = button(text(format!("Name{}", indicator(&SortField::Name))).size(11))
        .on_press(Message::SortBy(SortField::Name, next_dir(&SortField::Name)))
        .style(iced::widget::button::text)
        .width(Length::Fill)
        .padding([2, 4]);
    let kind = button(text(format!("Type{}", indicator(&SortField::Kind))).size(11))
        .on_press(Message::SortBy(SortField::Kind, next_dir(&SortField::Kind)))
        .style(iced::widget::button::text)
        .width(Length::Fixed(95.0))
        .padding([2, 4]);
    let size = button(text(format!("Size{}", indicator(&SortField::Size))).size(11))
        .on_press(Message::SortBy(SortField::Size, next_dir(&SortField::Size)))
        .style(iced::widget::button::text)
        .width(Length::Fixed(90.0))
        .padding([2, 4]);
    let modified = button(text(format!("Modified{}", indicator(&SortField::Modified))).size(11))
        .on_press(Message::SortBy(
            SortField::Modified,
            next_dir(&SortField::Modified),
        ))
        .style(iced::widget::button::text)
        .width(Length::Fixed(130.0))
        .padding([2, 4]);

    container(
        row![icon_spacer, name, kind, size, modified]
            .spacing(8)
            .padding([5, 10]),
    )
    .style(header_style)
    .into()
}

fn grouped_list(groups: &GroupedNodes, state: &AppState) -> Element<'static, Message> {
    let total: usize = groups.groups.iter().map(|g| g.nodes.len()).sum();
    if total == 0 {
        return empty_state("Empty folder");
    }

    let mut rows: Vec<Element<Message>> = Vec::new();
    for group in &groups.groups {
        if !group.label.is_empty() {
            rows.push(
                container(
                    text(group.label.clone())
                        .size(10)
                        .color(Color::from_rgb(0.50, 0.50, 0.50)),
                )
                .padding([4, 10])
                .into(),
            );
        }
        for node in &group.nodes {
            rows.push(file_row(node, state));
        }
    }

    scrollable(Column::with_children(rows).spacing(1).padding([0, 4]))
        .height(Length::Fill)
        .into()
}

fn flat_list(nodes: &[FileNode], state: &AppState) -> Element<'static, Message> {
    if nodes.is_empty() {
        return empty_state("No results");
    }

    let rows = nodes
        .iter()
        .map(|node| file_row(node, state))
        .collect::<Vec<_>>();
    scrollable(Column::with_children(rows).spacing(1).padding([0, 4]))
        .height(Length::Fill)
        .into()
}

fn file_row(node: &FileNode, state: &AppState) -> Element<'static, Message> {
    let selected = state.selection.contains(node.id);

    let icon = text(icons::for_node(node)).size(14);
    let name = text(node.name.clone()).size(12).width(Length::Fill);
    let kind = text(kind_label(&node.kind))
        .size(11)
        .width(Length::Fixed(95.0));
    let size_label = match &node.kind {
        NodeKind::Directory { .. } => "—".to_string(),
        _ => size_human(node.size),
    };
    let size = text(size_label).size(11).width(Length::Fixed(90.0));
    let modified = text(time_relative(node.modified))
        .size(11)
        .width(Length::Fixed(130.0));

    let content = row![icon, name, kind, size, modified]
        .spacing(8)
        .padding([7, 10])
        .align_y(iced::Alignment::Center);

    let row_btn = button(content)
        .on_press(Message::ActivateNode(node.id))
        .style(move |theme, status| row_style(theme, status, selected))
        .width(Length::Fill);
    mouse_area(row_btn)
        .on_move(Message::PointerMoved)
        .on_right_press(Message::OpenContextMenu(node.id))
        .into()
}

fn kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Directory { .. } => "Folder",
        NodeKind::File { .. } => "File",
        NodeKind::Symlink { .. } => "Symlink",
    }
}

fn row_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
    selected: bool,
) -> iced::widget::button::Style {
    let c = palette(theme);

    if selected {
        return iced::widget::button::Style {
            background: Some(iced::Background::Color(c.accent_soft)),
            text_color: c.text,
            border: iced::Border {
                color: c.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        };
    }

    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(iced::Background::Color(c.surface_alt)),
            text_color: c.text,
            border: iced::Border {
                color: c.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color: c.text,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
    }
}

fn header_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let c = palette(theme);
    iced::widget::container::Style {
        background: Some(iced::Background::Color(c.surface_alt)),
        text_color: Some(c.muted),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn empty_state(label: &str) -> Element<'static, Message> {
    container(
        text(label.to_string())
            .size(12)
            .color(Color::from_rgb(0.5, 0.55, 0.6)),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
