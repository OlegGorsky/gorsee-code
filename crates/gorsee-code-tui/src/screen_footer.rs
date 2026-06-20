use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{screen_parts::panel_block, theme};

pub(crate) fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let line = if area.width < 136 {
        compact_footer_line()
    } else {
        full_footer_line()
    };
    frame.render_widget(
        Paragraph::new(line)
            .block(panel_block(" "))
            .style(theme::normal()),
        area,
    );
}

fn full_footer_line() -> Line<'static> {
    Line::from(vec![
        Span::styled("[Enter]", theme::accent()),
        Span::raw(" запуск   "),
        Span::styled("[Shift+Enter/Ctrl+J]", theme::accent()),
        Span::raw(" строка   "),
        Span::styled("[Tab]", theme::accent()),
        Span::raw(" фокус   "),
        Span::styled("[/]", theme::accent()),
        Span::raw(" команды   "),
        Span::styled("[q]", theme::accent()),
        Span::raw(" выход"),
        Span::raw("        "),
        Span::styled(
            "Powered by Neurogate",
            theme::cyan().add_modifier(Modifier::BOLD),
        ),
    ])
}

fn compact_footer_line() -> Line<'static> {
    Line::from(vec![
        Span::styled("[Enter]", theme::accent()),
        Span::raw(" запуск   "),
        Span::styled("[/]", theme::accent()),
        Span::raw(" команды   "),
        Span::styled("[Ctrl+J]", theme::accent()),
        Span::raw(" строка   "),
        Span::styled("[Tab]", theme::accent()),
        Span::raw(" фокус   "),
        Span::styled("[q]", theme::accent()),
        Span::raw(" выход   "),
        Span::styled("Neurogate", theme::cyan().add_modifier(Modifier::BOLD)),
    ])
}
