use gorsee_code_ui_state::{AgentView, WorkspaceState};
use ratatui::text::{Line, Span};

use crate::{screen_parts::status_style, theme};

pub(crate) fn agent_lines(agent: &AgentView) -> Vec<Line<'static>> {
    let percent = percent(agent.tokens_used as f64, agent.tokens_limit as f64);
    vec![
        Line::from(vec![
            Span::styled("  ● ", status_style(&agent.status)),
            Span::styled(title(&agent.role), theme::strong()),
        ]),
        Line::from(vec![
            Span::styled("    ", theme::dim()),
            Span::styled(agent.model.clone(), theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("    ", theme::dim()),
            Span::styled(
                format!(
                    "{}/{} токенов  {:.0}%",
                    agent.tokens_used, agent.tokens_limit, percent
                ),
                theme::normal(),
            ),
        ]),
        Line::from(vec![
            Span::styled("    ", theme::dim()),
            Span::styled(bar(percent, 12), status_style(&agent.status)),
        ]),
    ]
}

pub(crate) fn limit_lines(state: &WorkspaceState) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled("ЛИМИТЫ", theme::accent())]),
        limit_numbers(
            "сессия",
            state.budget.used_tokens,
            state.budget.limit_tokens,
        ),
        limit_bar(state.budget.used_tokens, state.budget.limit_tokens),
        Line::from(vec![
            Span::styled("  live-окна ", theme::cyan()),
            Span::styled("/limits", theme::dim()),
        ]),
    ]
}

fn limit_numbers(label: &str, used: u64, limit: u64) -> Line<'static> {
    let percent = percent(used as f64, limit as f64);
    Line::from(vec![
        Span::styled(format!("  {label} "), theme::cyan()),
        Span::styled(format!("{used}/{limit} токенов"), theme::normal()),
        Span::styled(format!(" {:.0}%", percent), theme::dim()),
    ])
}

fn limit_bar(used: u64, limit: u64) -> Line<'static> {
    let percent = percent(used as f64, limit as f64);
    Line::from(vec![
        Span::styled("  ", theme::dim()),
        Span::styled(bar(percent, 14), theme::accent()),
    ])
}

fn percent(used: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        0.0
    } else {
        (used * 100.0 / limit).clamp(0.0, 100.0)
    }
}

fn bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn title(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
