use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenLayout {
    pub left: Rect,
    pub center: Rect,
    pub right: Rect,
    pub composer: Rect,
    pub footer: Rect,
}

pub fn screen_layout(area: Rect, composer_rows: u16) -> ScreenLayout {
    let footer_height = area.height.min(3);
    let composer_height = composer_rows.clamp(1, 12).saturating_add(2);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(composer_height.min(area.height.saturating_sub(footer_height))),
            Constraint::Length(footer_height),
        ])
        .split(area);

    let main = vertical[0];
    let footer = vertical[2];
    let composer = vertical[1];
    let left_width = if area.width >= 100 { 30 } else { 26 };
    let right_width = if area.width >= 120 { 32 } else { 0 };
    let columns = if right_width == 0 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(left_width), Constraint::Min(40)])
            .split(main)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_width),
                Constraint::Min(44),
                Constraint::Length(right_width),
            ])
            .split(main)
    };

    ScreenLayout {
        left: columns[0],
        center: columns[1],
        right: if right_width == 0 {
            Rect::default()
        } else {
            columns[2]
        },
        composer,
        footer,
    }
}
