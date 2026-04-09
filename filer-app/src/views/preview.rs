use filer_core::{ArchivePreviewEntry, FileNode, PreviewData};
use iced::widget::{column, container, image, row, scrollable, text, Column};
use iced::{ContentFit, Element, Length};

use crate::format::{size_human, time_relative};
use crate::message::Message;
use crate::state::AppState;

/// Build the preview panel.
pub fn view(state: &AppState, all_nodes: &[FileNode]) -> Element<'static, Message> {
    let inner: Element<Message> = match &state.preview_data {
        Some(PreviewData::Text { content, truncated, total_lines }) => {
            text_preview(content, *truncated, *total_lines)
        }
        Some(PreviewData::HighlightedText { content, truncated, .. }) => {
            // Display as plain text; syntax colours deferred to a later phase.
            text_preview(content, *truncated, 0)
        }
        Some(PreviewData::Image { data, width, height, .. }) => {
            image_preview(data, *width, *height)
        }
        Some(PreviewData::Audio { duration_secs, .. }) => {
            metadata_text(&format!("Audio — {:.0}s", duration_secs))
        }
        Some(PreviewData::Video { duration_secs, .. }) => {
            metadata_text(&format!("Video — {:.0}s", duration_secs))
        }
        Some(PreviewData::Archive { entries, total_entries, truncated }) => {
            archive_preview(entries, *total_entries, *truncated)
        }
        Some(PreviewData::Binary { hex_dump, size }) => {
            metadata_text(&format!("Binary — {}\n\n{}", size_human(*size), hex_dump))
        }
        Some(PreviewData::Document { total_pages, .. }) => {
            metadata_text(&format!("Document — {total_pages} pages"))
        }
        Some(PreviewData::Unsupported { mime_type, .. }) => {
            metadata_text(&format!("No preview for {mime_type}"))
        }
        None => {
            // Show basic FileNode metadata if a node is selected.
            if let Some(id) = state.preview_node {
                if let Some(node) = all_nodes.iter().find(|n| n.id == id) {
                    node_metadata_view(node)
                } else {
                    placeholder()
                }
            } else {
                placeholder()
            }
        }
    };

    container(inner)
        .width(Length::Fixed(300.0))
        .height(Length::Fill)
        .padding(12)
        .into()
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
        items.push(text(format!("… ({total} total)")).size(11).into());
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
        text(format!("{key}:")).size(12).width(Length::Fixed(70.0)),
        text(value.to_owned()).size(12),
    ]
    .spacing(4)
    .into()
}

fn placeholder() -> Element<'static, Message> {
    container(text("Select a file to preview").size(12))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
