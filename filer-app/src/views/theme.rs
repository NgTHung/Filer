use iced::widget::{button, container};
use iced::{Background, Color, Theme};

#[derive(Clone, Copy)]
pub struct UiPalette {
    pub app_bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub danger: Color,
}

pub fn palette(theme: &Theme) -> UiPalette {
    let p = theme.palette();
    let light = (p.background.r + p.background.g + p.background.b) / 3.0 > 0.5;
    if light {
        UiPalette {
            app_bg: Color::from_rgb8(0xF7, 0xF9, 0xFC),
            surface: Color::from_rgb8(0xFF, 0xFF, 0xFF),
            surface_alt: Color::from_rgb8(0xF1, 0xF4, 0xF8),
            border: Color::from_rgb8(0xD8, 0xDE, 0xE8),
            text: Color::from_rgb8(0x1F, 0x29, 0x37),
            muted: Color::from_rgb8(0x6B, 0x72, 0x80),
            accent: Color::from_rgb8(0x25, 0x63, 0xEB),
            accent_soft: Color::from_rgb8(0xDB, 0xEA, 0xFE),
            danger: Color::from_rgb8(0xDC, 0x26, 0x26),
        }
    } else {
        UiPalette {
            app_bg: Color::from_rgb8(0x15, 0x17, 0x1A),
            surface: Color::from_rgb8(0x1E, 0x20, 0x24),
            surface_alt: Color::from_rgb8(0x28, 0x2B, 0x30),
            border: Color::from_rgb8(0x3A, 0x3F, 0x46),
            text: Color::from_rgb8(0xE8, 0xEA, 0xED),
            muted: Color::from_rgb8(0x9A, 0xA1, 0xAA),
            accent: Color::from_rgb8(0x4C, 0x8D, 0xFF),
            accent_soft: Color::from_rgb8(0x23, 0x3A, 0x5E),
            danger: Color::from_rgb8(0xF8, 0x71, 0x71),
        }
    }
}

pub fn panel_surface(theme: &Theme) -> container::Style {
    let c = palette(theme);
    container::Style {
        background: Some(Background::Color(c.surface)),
        border: iced::Border {
            color: c.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: Some(c.text),
        snap: false,
    }
}

pub fn panel_alt_surface(theme: &Theme) -> container::Style {
    let c = palette(theme);
    container::Style {
        background: Some(Background::Color(c.surface_alt)),
        border: iced::Border {
            color: c.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: Some(c.text),
        snap: false,
    }
}

pub fn top_button(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let c = palette(theme);
    match (active, status) {
        (true, _) => button::Style {
            background: Some(Background::Color(c.accent_soft)),
            text_color: c.accent,
            border: iced::Border {
                color: c.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
        (false, button::Status::Hovered) => button::Style {
            background: Some(Background::Color(c.surface_alt)),
            text_color: c.text,
            border: iced::Border {
                color: c.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
        _ => button::Style {
            background: Some(Background::Color(c.surface)),
            text_color: c.text,
            border: iced::Border {
                color: c.border,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow::default(),
            snap: false,
        },
    }
}
