use gorsee_code_ui_state::EventView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    attachment::AttachmentKind, completion::CompletionKind, project::ProjectEntry, theme,
    WorkspaceApp,
};

pub(crate) fn panel(lines: Vec<Line<'static>>, title: impl Into<String>) -> Paragraph<'static> {
    Paragraph::new(lines)
        .block(panel_block(title))
        .style(theme::normal())
        .wrap(Wrap { trim: false })
}

pub(crate) fn panel_block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .title(title.into())
        .borders(Borders::ALL)
        .border_style(theme::border())
        .style(ratatui::style::Style::default().bg(theme::panel_bg()))
}

pub(crate) fn project_lines(app: &WorkspaceApp) -> Vec<Line<'static>> {
    let entries = app.project_entries();
    if entries.is_empty() {
        return vec![Line::from(vec![Span::styled(
            "  проект пока пуст",
            theme::dim(),
        )])];
    }

    let start = app.project_scroll().min(entries.len());
    entries
        .iter()
        .enumerate()
        .skip(start)
        .take(12)
        .map(|(index, entry)| project_line(entry, index == app.project_selected()))
        .collect()
}

pub(crate) fn input_lines(app: &WorkspaceApp) -> Vec<Line<'static>> {
    let cursor = app.input_cursor().min(app.input().len());
    let before = &app.input()[..cursor];
    let after = &app.input()[cursor..];
    input_lines_from_parts(before, after)
}

pub(crate) fn attachment_lines(app: &WorkspaceApp) -> Vec<Line<'static>> {
    if app.attachments().is_empty() {
        return Vec::new();
    }
    let mut lines = vec![Line::from(vec![Span::styled(
        "Вложения",
        theme::accent().add_modifier(Modifier::BOLD),
    )])];
    lines.extend(app.attachments().iter().map(|attachment| {
        let icon = match attachment.kind() {
            AttachmentKind::Image => "󰥶",
            AttachmentKind::File => "",
        };
        Line::from(vec![
            Span::styled(format!("  {icon} "), theme::cyan()),
            Span::styled(attachment.label().to_string(), theme::normal()),
        ])
    }));
    lines
}

pub(crate) fn editor_lines(text: &str, cursor: usize, scroll: usize) -> Vec<Line<'static>> {
    if text.is_empty() {
        return vec![Line::from(vec![Span::styled("▌", theme::accent())])];
    }
    let cursor = cursor.min(text.len());
    let before = &text[..cursor];
    let after = &text[cursor..];
    let mut lines = input_lines_from_parts(before, after);
    lines = lines.into_iter().skip(scroll).collect();
    lines.truncate(28);
    lines
}

pub(crate) fn completion_lines(app: &WorkspaceApp) -> Vec<Line<'static>> {
    let title = match app.completion_kind() {
        Some(CompletionKind::Commands) => "команды",
        Some(CompletionKind::Files) => "файлы",
        None => "подсказки",
    };
    let mut lines = vec![Line::from(vec![Span::styled(
        format!("  {title}"),
        theme::accent(),
    )])];
    lines.extend(
        app.completion_items()
            .iter()
            .enumerate()
            .skip(app.completion_visible_start())
            .take(8)
            .map(|(index, item)| {
                let selected = index == app.completion_selected();
                let marker = if selected { "▌" } else { " " };
                let style = if selected {
                    theme::cyan().add_modifier(Modifier::BOLD)
                } else {
                    theme::normal()
                };
                Line::from(vec![
                    Span::styled(format!(" {marker} "), theme::dim()),
                    Span::styled(item.label().to_string(), style),
                    Span::styled(format!("  {}", item.detail()), theme::dim()),
                ])
            }),
    );
    lines
}

pub(crate) fn push_event(lines: &mut Vec<Line<'static>>, event: &EventView) {
    lines.push(Line::from(vec![
        Span::styled(format!("#{:04} ", event.sequence), theme::dim()),
        Span::styled(label(&event.kind), theme::cyan()),
        Span::raw(" "),
        Span::styled(
            title(event.agent_id.as_deref().unwrap_or("workspace")),
            theme::strong(),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("       │ ", theme::dim()),
        Span::raw(summary(event)),
    ]));
}

pub(crate) fn status_label(status: &str) -> String {
    match status {
        "running" => "идет".into(),
        "finished" => "завершена".into(),
        "ready" => "готово".into(),
        "waiting_approval" => "ждет подтверждения".into(),
        "paused" => "пауза".into(),
        "failed" => "ошибка".into(),
        _ => status.replace('_', " "),
    }
}

pub(crate) fn status_style(status: &str) -> Style {
    match status {
        "running" | "planning" | "reading" | "patching" | "validating" => theme::cyan(),
        "finished" | "ready" => theme::green(),
        "waiting_approval" | "paused" => theme::warning(),
        "failed" => theme::warning().add_modifier(Modifier::BOLD),
        _ => theme::dim(),
    }
}

fn project_line(entry: &ProjectEntry, selected: bool) -> Line<'static> {
    let marker = if selected { "▌" } else { " " };
    let indent = "  ".repeat(entry.depth());
    let icon = if entry.is_dir() {
        if entry.is_expanded() {
            ""
        } else {
            ""
        }
    } else {
        ""
    };
    let label = entry
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| entry.path().to_str().unwrap_or("?"));
    let style = if selected {
        theme::cyan().add_modifier(Modifier::BOLD)
    } else if entry.is_dir() {
        theme::accent()
    } else {
        theme::normal()
    };
    Line::from(vec![
        Span::styled(format!(" {marker} {indent}"), theme::dim()),
        Span::styled(icon, theme::cyan()),
        Span::raw(" "),
        Span::styled(label.to_string(), style),
    ])
}

fn input_lines_from_parts(before: &str, after: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut before_lines = before.split('\n').collect::<Vec<_>>();
    let after_lines = after.split('\n').collect::<Vec<_>>();
    let cursor_line = before_lines.pop().unwrap_or("");
    for line in before_lines {
        lines.push(code_line(line));
    }
    let mut current = vec![
        Span::styled(cursor_line.to_string(), theme::normal()),
        Span::styled("▌", theme::accent()),
    ];
    if let Some(first_after) = after_lines.first() {
        current.push(Span::styled(first_after.to_string(), theme::normal()));
    }
    lines.push(Line::from(current));
    for line in after_lines.iter().skip(1) {
        lines.push(code_line(line));
    }
    lines
}

fn code_line(line: &str) -> Line<'static> {
    Line::from(vec![Span::styled(line.to_string(), theme::normal())])
}

fn label(kind: &str) -> String {
    match kind {
        "event" => "событие".into(),
        "session_started" => "сессия начата".into(),
        "session_finished" => "сессия завершена".into(),
        "tool_call" => "tool call".into(),
        "tool_result" => "результат tool".into(),
        "approval_requested" => "запрос подтверждения".into(),
        "approval_decided" => "решение подтверждения".into(),
        _ => kind.replace('_', " "),
    }
}

fn summary(event: &EventView) -> String {
    let raw = event.summary.trim();
    if raw.is_empty() || raw == event.kind {
        label(&event.kind)
    } else {
        raw.to_string()
    }
}

fn title(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
