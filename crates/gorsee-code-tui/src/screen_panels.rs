use ratatui::{
    layout::Rect,
    text::{Line, Span},
    Frame,
};

use crate::{screen_parts::panel, theme, WorkspaceApp};

pub(crate) fn render_sessions_panel(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let mut lines = vec![
        Line::from(vec![Span::styled("Сессии", theme::strong())]),
        Line::from(vec![Span::styled(
            "Enter выбрать  ↑/↓ навигация",
            theme::dim(),
        )]),
        Line::raw(""),
    ];
    if app.sessions.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  сессий пока нет",
            theme::dim(),
        )]));
    } else {
        lines.extend(
            app.sessions
                .iter()
                .enumerate()
                .map(|(index, id)| selected_line(id, index == app.selected_session)),
        );
    }
    frame.render_widget(panel(lines, " Сессии "), area);
}

pub(crate) fn render_models_panel(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let mut lines = vec![
        Line::from(vec![Span::styled("Модели агентов", theme::strong())]),
        Line::from(vec![Span::styled(
            "←/→ модель  ↑/↓ агент  Enter сохранить",
            theme::dim(),
        )]),
        Line::raw(""),
    ];
    if app.models.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  агенты не найдены",
            theme::dim(),
        )]));
    } else {
        lines.extend(app.models.iter().enumerate().map(|(index, choice)| {
            selected_line(
                &format!("{}  {}", choice.agent(), choice.model()),
                index == app.selected_model,
            )
        }));
    }
    frame.render_widget(panel(lines, " Модели "), area);
}

fn selected_line(value: &str, selected: bool) -> Line<'static> {
    let marker = if selected { "▌" } else { " " };
    let style = if selected {
        theme::cyan()
    } else {
        theme::normal()
    };
    Line::from(vec![
        Span::styled(format!(" {marker} "), theme::dim()),
        Span::styled(value.to_string(), style),
    ])
}
