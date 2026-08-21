use iced::widget::{button, container};
use iced::{Background, Color, Theme};

pub const COLOR_BG_WINDOW: Color = Color::from_rgb(0.05, 0.05, 0.05);
pub const COLOR_BG_HEADER: Color = Color::from_rgb(0.2, 0.2, 0.2);
pub const COLOR_BG_CELL: Color = Color::from_rgb(0.1, 0.1, 0.1);
pub const COLOR_BG_CELL_ALT: Color = Color::from_rgb(0.07, 0.07, 0.07);
pub const COLOR_BG_SELECTED: Color = Color::from_rgb(0.1, 0.4, 0.8);
pub const COLOR_ACCENT: Color = Color::from_rgb(0.0, 0.5, 1.0);
pub const COLOR_TEXT_PRIMARY: Color = Color::WHITE;
pub const COLOR_TEXT_DIM: Color = Color::from_rgb(0.6, 0.6, 0.6);
pub const COLOR_HANDLE: Color = Color::from_rgb(0.4, 0.4, 0.4);
pub const COLOR_ERROR: Color = Color::from_rgb(1.0, 0.4, 0.4);
pub const COLOR_SUCCESS: Color = Color::from_rgb(0.4, 1.0, 0.4);

pub fn header_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG_HEADER)),
        text_color: Some(COLOR_TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn window_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(COLOR_BG_WINDOW)),
        text_color: Some(COLOR_TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn cell_container_style(selected: bool, alt: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(if selected {
            COLOR_BG_SELECTED
        } else if alt {
            COLOR_BG_CELL_ALT
        } else {
            COLOR_BG_CELL
        })),
        text_color: Some(COLOR_TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn tool_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Background::Color(COLOR_BG_HEADER)),
        text_color: COLOR_TEXT_PRIMARY,
        ..Default::default()
    };
    match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(COLOR_BG_SELECTED)),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(COLOR_ACCENT)),
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: COLOR_TEXT_DIM,
            ..base
        },
        _ => base,
    }
}

pub fn transparent_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: COLOR_TEXT_PRIMARY,
        ..Default::default()
    }
}
