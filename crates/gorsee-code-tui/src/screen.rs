use gorsee_code_ui_state::WorkspaceState;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use crate::{
    layout::screen_layout,
    navigation::{FocusPane, MENU_ITEMS},
    project::path_label,
    screen_footer::render_footer,
    screen_metrics::{agent_lines, limit_lines},
    screen_panels::{render_models_panel, render_sessions_panel},
    screen_parts::{
        attachment_lines, completion_lines, editor_lines, input_lines, panel, panel_block,
        project_lines, push_event, status_label, status_style,
    },
    theme, CenterPanel, WorkspaceApp,
};

pub fn render_frame(frame: &mut Frame<'_>, state: &WorkspaceState, app: &WorkspaceApp) {
    let layout = screen_layout(frame.area(), app.composer_rows());
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::panel_bg())),
        frame.area(),
    );
    render_sidebar(frame, layout.left, state, app);
    render_center(frame, layout.center, state, app);
    if layout.right.width > 0 {
        render_context(frame, layout.right, state, app);
    }
    render_composer(frame, layout.composer, app);
    render_footer(frame, layout.footer);
}

fn render_sidebar(frame: &mut Frame<'_>, area: Rect, state: &WorkspaceState, app: &WorkspaceApp) {
    let mut lines = vec![Line::from(vec![Span::styled("МЕНЮ ─", theme::accent())])];
    lines.extend(MENU_ITEMS.iter().enumerate().map(|(index, item)| {
        let selected = app.focus_pane() == FocusPane::Menu && index == app.selected_menu_index();
        menu_line(item.icon, item.label, selected)
    }));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![Span::styled("ФАЙЛЫ ─", theme::accent())]));
    lines.push(Line::from(vec![
        Span::styled("   ", theme::cyan()),
        Span::raw(folder_name(app, state)),
    ]));
    lines.extend(project_lines(app));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  сессия ", theme::dim()),
        Span::styled(
            status_label(&state.session.status),
            status_style(&state.session.status),
        ),
    ]));

    frame.render_widget(panel(lines, " "), area);
}

fn render_center(frame: &mut Frame<'_>, area: Rect, state: &WorkspaceState, app: &WorkspaceApp) {
    if app.is_editor_open() {
        render_editor(frame, area, app);
    } else if app.center_panel() == CenterPanel::Sessions && app.output().is_none() {
        render_sessions_panel(frame, area, app);
    } else if app.center_panel() == CenterPanel::Models && app.output().is_none() {
        render_models_panel(frame, area, app);
    } else if app.output().is_some() && app.center_panel() != CenterPanel::Timeline {
        render_output_panel(frame, area, app);
    } else {
        render_timeline(frame, area, state, app);
    }
}

fn render_timeline(frame: &mut Frame<'_>, area: Rect, state: &WorkspaceState, app: &WorkspaceApp) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Gorsee Code", theme::strong()),
            Span::raw(" - "),
            Span::styled("Лента", theme::cyan()),
            Span::raw("  "),
            Span::styled(state.session.id.clone(), theme::dim()),
        ]),
        Line::raw(""),
    ];

    let end = state
        .timeline
        .len()
        .saturating_sub(app.center_scroll())
        .max(1)
        .min(state.timeline.len());
    let start = end.saturating_sub(12);
    for event in &state.timeline[start..end] {
        push_event(&mut lines, event);
    }

    if let Some(output) = app.output() {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled("Вывод", theme::accent())]));
        lines.extend(
            output
                .lines()
                .take(8)
                .map(|line| Line::raw(format!("  {line}"))),
        );
    }

    frame.render_widget(panel(lines, " Лента "), area);
}

fn render_editor(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let Some(editor) = app.editor() else {
        return;
    };
    let dirty = if editor.is_dirty() {
        "изменен"
    } else {
        "сохранен"
    };
    let title = format!(" Файл: {}  {} ", path_label(editor.path()), dirty);
    let mut lines = vec![Line::from(vec![
        Span::styled("Ctrl+S", theme::accent()),
        Span::raw(" сохранить   "),
        Span::styled("Ctrl+W", theme::accent()),
        Span::raw(" закрыть"),
    ])];
    lines.push(Line::raw(""));
    lines.extend(editor_lines(
        editor.text(),
        editor.cursor(),
        editor.scroll(),
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(title))
            .style(theme::normal())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_output_panel(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let title = match app.center_panel() {
        CenterPanel::Diff => " Diff ",
        CenterPanel::Sessions => " Сессии ",
        CenterPanel::Models => " Модели ",
        CenterPanel::Instructions => " Инструкции ",
        CenterPanel::Skills => " Скиллы ",
        CenterPanel::Mcp => " MCP ",
        CenterPanel::Limits => " Лимиты ",
        CenterPanel::Terminal => " Терминал ",
        CenterPanel::Timeline => " Вывод ",
    };
    let lines = app
        .output()
        .unwrap_or_default()
        .lines()
        .skip(app.center_scroll())
        .take(32)
        .map(|line| {
            let style = if line.starts_with('+') {
                theme::green()
            } else if line.starts_with('-') {
                theme::warning()
            } else if line.starts_with('$') {
                theme::cyan()
            } else {
                theme::normal()
            };
            Line::from(vec![Span::styled(line.to_string(), style)])
        })
        .collect::<Vec<_>>();
    frame.render_widget(panel(lines, title), area);
}

fn render_context(frame: &mut Frame<'_>, area: Rect, state: &WorkspaceState, app: &WorkspaceApp) {
    let mut lines = vec![
        Line::from(vec![Span::styled("КОНТЕКСТ", theme::accent())]),
        Line::raw(""),
    ];
    if is_working(app) {
        lines.push(Line::from(vec![
            Span::styled(thinking_frame(app.spinner_tick()), theme::cyan()),
            Span::styled(" Думаю...", theme::accent()),
        ]));
        lines.push(Line::raw(""));
    }
    for agent in &state.agents {
        lines.extend(agent_lines(agent));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![Span::styled(
        "ПОДТВЕРЖДЕНИЯ",
        theme::accent(),
    )]));
    if state.approvals.is_empty() {
        lines.push(Line::from(vec![Span::styled("  нет", theme::dim())]));
    } else {
        for approval in &state.approvals {
            lines.push(Line::raw(format!("  {} {}", approval.id, approval.name)));
            lines.push(Line::from(vec![
                Span::styled("  risk ", theme::dim()),
                Span::styled(approval.risk.clone(), theme::warning()),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.extend(limit_lines(state));

    frame.render_widget(panel(lines, " Контекст "), area);
}

fn render_composer(frame: &mut Frame<'_>, area: Rect, app: &WorkspaceApp) {
    let mut text = if app.input().is_empty() {
        vec![Line::from(vec![Span::styled(
            "Введите задачу или команду...",
            theme::dim(),
        )])]
    } else {
        input_lines(app)
    };
    if !app.completion_items().is_empty() {
        text.push(Line::raw(""));
        text.extend(completion_lines(app));
    }
    let attachments = attachment_lines(app);
    if !attachments.is_empty() {
        text.push(Line::raw(""));
        text.extend(attachments);
    }
    frame.render_widget(
        Paragraph::new(text)
            .block(panel_block(" Введите задачу "))
            .style(theme::normal())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn menu_line(icon: &'static str, label: &'static str, selected: bool) -> Line<'static> {
    let marker = if selected { "▌" } else { " " };
    let style = if selected {
        theme::cyan().add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        theme::normal()
    };
    Line::from(vec![
        Span::styled(format!("{marker} {icon}"), theme::cyan()),
        Span::raw(" "),
        Span::styled(label.to_string(), style),
    ])
}

fn folder_name(app: &WorkspaceApp, state: &WorkspaceState) -> String {
    app.working_folder()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| path.to_str().unwrap_or("project"))
                .to_string()
        })
        .unwrap_or_else(|| repo_name(state))
}

fn repo_name(state: &WorkspaceState) -> String {
    state
        .session
        .repo
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("project")
        .to_string()
}

fn is_working(app: &WorkspaceApp) -> bool {
    matches!(
        app.status(),
        Some("running" | "working" | "approving" | "denying" | "pausing" | "resuming")
    )
}

fn thinking_frame(tick: u64) -> &'static str {
    ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][(tick as usize) % 8]
}
