use filer_core::model::node::NodeKind;
use filer_core::{ArchivePreviewEntry, FileNode, PreviewData};
use iced::widget::{Column, button, column, container, image, row, rule, scrollable, text};
use iced::{Color, ContentFit, Element, Length};

use crate::format::{size_human, time_relative};
use crate::message::Message;
use crate::state::{AppState, PanelTab};
use crate::views::theme::{panel_alt_surface, top_button};

pub fn view(state: &AppState) -> Element<'static, Message> {
    let all_nodes = state.visible_nodes();
    let selected = state
        .preview_node
        .and_then(|id| all_nodes.iter().find(|n| n.id == id).cloned());

    let details_tab = tab_button("Details", PanelTab::Details, state.panel_tab);
    let preview_tab = tab_button("Preview", PanelTab::Preview, state.panel_tab);
    let tabs = row![details_tab, preview_tab].spacing(4);

    let content = match state.panel_tab {
        PanelTab::Details => {
            if let Some(node) = &selected {
                node_metadata_view(node)
            } else {
                placeholder("Select a file or folder")
            }
        }
        PanelTab::Preview => preview_content(&selected, state.preview_data.as_ref()),
    };

    let title = selected
        .as_ref()
        .map(|node| node.name.clone())
        .unwrap_or_else(|| "Details".to_string());

    container(column![text(title).size(12), tabs, rule::horizontal(1), content,].spacing(6))
        .width(Length::Fixed(320.0))
        .height(Length::Fill)
        .padding(12)
        .style(panel_style)
        .into()
}

fn tab_button(label: &'static str, tab: PanelTab, active: PanelTab) -> Element<'static, Message> {
    let style = if tab == active { true } else { false };
    button(text(label.to_string()).size(11))
        .on_press(Message::SetPanelTab(tab))
        .style(move |theme, status| top_button(theme, status, style))
        .padding([4, 8])
        .into()
}

fn preview_content(
    selected: &Option<FileNode>,
    preview: Option<&PreviewData>,
) -> Element<'static, Message> {
    match preview {
        Some(PreviewData::Text {
            content,
            truncated,
            total_lines,
        }) => text_preview(content, *truncated, *total_lines),
        Some(PreviewData::HighlightedText {
            content,
            language,
            truncated,
            ..
        }) => {
            let plain = strip_html_tags(content);
            column![
                text(format!("Code ({language})"))
                    .size(11)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
                text_preview(&plain, *truncated, 0),
            ]
            .spacing(4)
            .into()
        }
        Some(PreviewData::Image {
            data,
            width,
            height,
            ..
        }) => image_preview(data, *width, *height),
        Some(PreviewData::Audio { duration_secs, .. }) => {
            metadata_text(&format!("Audio ({duration_secs:.0}s)"))
        }
        Some(PreviewData::Video { duration_secs, .. }) => {
            metadata_text(&format!("Video ({duration_secs:.0}s)"))
        }
        Some(PreviewData::Archive {
            entries,
            total_entries,
            truncated,
        }) => archive_preview(entries, *total_entries, *truncated),
        Some(PreviewData::Binary { hex_dump, size }) => {
            metadata_text(&format!("Binary ({})\n\n{}", size_human(*size), hex_dump))
        }
        Some(PreviewData::Document { total_pages, .. }) => {
            metadata_text(&format!("Document ({total_pages} pages)"))
        }
        Some(PreviewData::Unsupported { mime_type, .. }) => {
            metadata_text(&format!("No preview for {mime_type}"))
        }
        None => {
            if let Some(node) = selected {
                if matches!(node.kind, NodeKind::Directory { .. }) {
                    return placeholder("No folder preview. Open Details for metadata.");
                }
                placeholder("Loading preview...")
            } else {
                placeholder("Select a file to preview")
            }
        }
    }
}

fn text_preview(content: &str, truncated: bool, _total_lines: usize) -> Element<'static, Message> {
    let footer = if truncated { "\n[truncated]" } else { "" };
    scrollable(
        text(format!("{content}{footer}"))
            .font(iced::Font::MONOSPACE)
            .size(11),
    )
    .height(Length::Fill)
    .into()
}

fn image_preview(data: &[u8], _w: u32, _h: u32) -> Element<'static, Message> {
    let handle = image::Handle::from_bytes(data.to_vec());
    image(handle)
        .content_fit(ContentFit::Contain)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn archive_preview(
    entries: &[ArchivePreviewEntry],
    total: usize,
    truncated: bool,
) -> Element<'static, Message> {
    let mut items: Vec<Element<Message>> = entries
        .iter()
        .map(|e| {
            let icon = if e.is_directory { "📁" } else { "📄" };
            text(format!("{icon} {} ({})", e.path, size_human(e.size)))
                .size(11)
                .into()
        })
        .collect();

    if truncated {
        items.push(text(format!("... ({total} total entries)")).size(11).into());
    }

    scrollable(Column::with_children(items).spacing(2))
        .height(Length::Fill)
        .into()
}

fn metadata_text(s: &str) -> Element<'static, Message> {
    scrollable(text(s.to_owned()).size(12))
        .height(Length::Fill)
        .into()
}

fn node_metadata_view(node: &FileNode) -> Element<'static, Message> {
    let rows = column![
        kv("Name", &node.name),
        kv("Size", &size_human(node.size)),
        kv("Modified", &time_relative(node.modified)),
        kv(
            "Kind",
            match &node.kind {
                filer_core::model::node::NodeKind::Directory { .. } => "Directory",
                filer_core::model::node::NodeKind::File { .. } => "File",
                filer_core::model::node::NodeKind::Symlink { .. } => "Symlink",
            }
        ),
    ]
    .spacing(4);
    scrollable(rows).height(Length::Fill).into()
}

fn kv(key: &str, value: &str) -> Element<'static, Message> {
    row![
        text(format!("{key}:")).size(11).width(Length::Fixed(72.0)),
        text(value.to_owned()).size(11),
    ]
    .spacing(4)
    .into()
}

fn placeholder(label: &str) -> Element<'static, Message> {
    container(text(label.to_string()).size(12))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn panel_style(theme: &iced::Theme) -> iced::widget::container::Style {
    panel_alt_surface(theme)
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
