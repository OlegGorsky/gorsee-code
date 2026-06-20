use ratatui::style::{Color, Modifier, Style};

pub fn normal() -> Style {
    Style::default().fg(Color::Rgb(188, 197, 232))
}

pub fn dim() -> Style {
    Style::default().fg(Color::Rgb(116, 126, 166))
}

pub fn accent() -> Style {
    Style::default().fg(Color::Rgb(138, 92, 246))
}

pub fn cyan() -> Style {
    Style::default().fg(Color::Rgb(65, 156, 255))
}

pub fn green() -> Style {
    Style::default().fg(Color::Rgb(46, 213, 115))
}

pub fn warning() -> Style {
    Style::default().fg(Color::Rgb(245, 158, 11))
}

pub fn strong() -> Style {
    normal().add_modifier(Modifier::BOLD)
}

pub fn selection() -> Style {
    Style::default()
        .fg(Color::Rgb(248, 250, 255))
        .bg(Color::Rgb(76, 60, 180))
        .add_modifier(Modifier::BOLD)
}

pub fn border() -> Style {
    Style::default().fg(Color::Rgb(48, 57, 104))
}

pub fn panel_bg() -> Color {
    Color::Rgb(4, 9, 28)
}
