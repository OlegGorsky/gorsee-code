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
            "Enter выбрать  ↑/↓ навигация  Новая сессия сбрасывает активную историю",
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
        lines.extend(app.sessions.iter().enumerate().map(|(index, item)| {
            selected_line(
                &format!("{}  {}", item.label(), item.detail()),
                index == app.selected_session,
            )
        }));
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

pub(crate) fn render_item_panel(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let (title, help, empty) = item_panel_text(app);
    let mut lines = vec![
        Line::from(vec![Span::styled(title, theme::strong())]),
        Line::from(vec![Span::styled(help, theme::dim())]),
        Line::raw(""),
    ];
    if app.panel_items().is_empty() {
        lines.push(Line::from(vec![Span::styled(empty, theme::dim())]));
    } else {
        lines.extend(app.panel_items().iter().enumerate().map(|(index, item)| {
            selected_line(
                &format!("{}  {}", item.label(), item.detail()),
                index == app.selected_panel_item(),
            )
        }));
    }
    frame.render_widget(panel(lines, format!(" {title} ")), area);
}

fn item_panel_text(app: &WorkspaceApp) -> (&'static str, &'static str, &'static str) {
    match app.center_panel() {
        crate::CenterPanel::Project => (
            "Проект",
            "Enter выбрать папку  ↑/↓ навигация  /project <путь>",
            "  проект не выбран",
        ),
        crate::CenterPanel::Instructions => (
            "Инструкции",
            "Enter открыть/создать  ↑/↓ навигация  Ctrl+S сохранить",
            "  инструкций нет",
        ),
        crate::CenterPanel::Skills => (
            "Скиллы",
            "Enter открыть проектный skill-файл  ↑/↓ навигация",
            "  скиллов нет",
        ),
        crate::CenterPanel::Mcp => (
            "MCP",
            "Enter открыть/создать config  ↑/↓ навигация",
            "  MCP config не найден",
        ),
        crate::CenterPanel::Limits => (
            "Лимиты",
            "Enter обновить live-лимиты  ↑/↓ навигация",
            "  лимитов нет",
        ),
        _ => ("Раздел", "↑/↓ навигация", "  пусто"),
    }
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
