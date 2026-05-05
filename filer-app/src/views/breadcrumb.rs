use std::path::{Component, Path, PathBuf};

use iced::widget::{button, row, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::views::theme::top_button;

/// Build a clickable breadcrumb bar from the current path.
pub fn view(path: &Path) -> Element<'static, Message> {
    let mut items: Vec<Element<Message>> = Vec::new();
    let mut acc = PathBuf::new();

    for component in path.components() {
        acc.push(component.as_os_str());
        let label = match component {
            Component::RootDir => "/".to_string(),
            Component::Normal(s) => s.to_string_lossy().into_owned(),
            Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
            _ => continue,
        };
        let target = acc.clone();
        items.push(
            button(text(label).size(13))
                .on_press(Message::Navigate(target))
                .padding([2, 6])
                .style(|theme, status| top_button(theme, status, false))
                .into(),
        );
        items.push(text(" › ").size(13).into());
    }
    if !items.is_empty() {
        items.pop(); // remove trailing separator
    }

    row(items).width(Length::Fill).spacing(0).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_breadcrumb_segments() {
        // Just verify it doesn't panic for a typical path.
        let path = PathBuf::from("/home/user/documents");
        let _ = view(&path);
    }
}
