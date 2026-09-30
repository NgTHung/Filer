// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// dafec0663843d071c195f3ad4d578afc06d68306b605cd6479c50cd54e527eb1
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("archive", "\u{E041}"),
    ("back", "\u{E048}"),
    ("bookmark", "\u{E23D}"),
    ("close", "\u{E1B2}"),
    ("copy", "\u{E09E}"),
    ("cut", "\u{E14E}"),
    ("file", "\u{E0C0}"),
    ("folder", "\u{E0D7}"),
    ("folder_plus", "\u{E0D9}"),
    ("forward", "\u{E049}"),
    ("image", "\u{E0F6}"),
    ("info", "\u{E0F9}"),
    ("moon", "\u{E11E}"),
    ("music", "\u{E122}"),
    ("panel_right", "\u{E431}"),
    ("paste", "\u{E3E8}"),
    ("pencil", "\u{E1F9}"),
    ("refresh", "\u{E145}"),
    ("search", "\u{E151}"),
    ("sun", "\u{E178}"),
    ("trash", "\u{E18E}"),
    ("up", "\u{E04A}"),
    ("video", "\u{E1A5}"),
];

pub fn archive<'a>() -> Text<'a> {
    icon("\u{E041}")
}

pub fn back<'a>() -> Text<'a> {
    icon("\u{E048}")
}

pub fn bookmark<'a>() -> Text<'a> {
    icon("\u{E23D}")
}

pub fn close<'a>() -> Text<'a> {
    icon("\u{E1B2}")
}

pub fn copy<'a>() -> Text<'a> {
    icon("\u{E09E}")
}

pub fn cut<'a>() -> Text<'a> {
    icon("\u{E14E}")
}

pub fn file<'a>() -> Text<'a> {
    icon("\u{E0C0}")
}

pub fn folder<'a>() -> Text<'a> {
    icon("\u{E0D7}")
}

pub fn folder_plus<'a>() -> Text<'a> {
    icon("\u{E0D9}")
}

pub fn forward<'a>() -> Text<'a> {
    icon("\u{E049}")
}

pub fn image<'a>() -> Text<'a> {
    icon("\u{E0F6}")
}

pub fn info<'a>() -> Text<'a> {
    icon("\u{E0F9}")
}

pub fn moon<'a>() -> Text<'a> {
    icon("\u{E11E}")
}

pub fn music<'a>() -> Text<'a> {
    icon("\u{E122}")
}

pub fn panel_right<'a>() -> Text<'a> {
    icon("\u{E431}")
}

pub fn paste<'a>() -> Text<'a> {
    icon("\u{E3E8}")
}

pub fn pencil<'a>() -> Text<'a> {
    icon("\u{E1F9}")
}

pub fn refresh<'a>() -> Text<'a> {
    icon("\u{E145}")
}

pub fn search<'a>() -> Text<'a> {
    icon("\u{E151}")
}

pub fn sun<'a>() -> Text<'a> {
    icon("\u{E178}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{E18E}")
}

pub fn up<'a>() -> Text<'a> {
    icon("\u{E04A}")
}

pub fn video<'a>() -> Text<'a> {
    icon("\u{E1A5}")
}

/// Render any Lucide icon by its codepoint string.
/// Use this together with [`ALL_ICONS`] to display icons dynamically:
/// ```ignore
/// for (name, cp) in ALL_ICONS {
///     button(render(cp)).on_press(Msg::Pick(name.to_string()))
/// }
/// ```
pub fn render(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("lucide"))
}

fn icon(codepoint: &str) -> Text<'_> {
    render(codepoint)
}
