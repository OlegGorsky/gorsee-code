use gorsee_code_ui_state::{AgentView, WorkspaceState};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{screen_parts::status_style, theme};

pub(crate) fn agent_lines(agent: &AgentView) -> Vec<Line<'static>> {
    let percent = percent(agent.tokens_used as f64, agent.tokens_limit as f64);
    let mut lines = vec![
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
        bar_line("    ", percent, 12, status_style(&agent.status)),
    ];
    if agent.cached_tokens > 0 {
        lines.push(Line::from(vec![
            Span::styled("    cache ", theme::dim()),
            Span::styled(format!("{} токенов", agent.cached_tokens), theme::dim()),
        ]));
    }
    lines
}

pub(crate) fn limit_lines(state: &WorkspaceState) -> Vec<Line<'static>> {
    let mut lines = vec![
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
    ];
    if state.budget.cached_tokens > 0 {
        lines.insert(
            3,
            Line::from(vec![
                Span::styled("  cache ", theme::dim()),
                Span::styled(
                    format!("{} токенов не списано", state.budget.cached_tokens),
                    theme::dim(),
                ),
            ]),
        );
    }
    lines
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
    bar_line("  ", percent, 14, theme::accent())
}

fn percent(used: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        0.0
    } else {
        (used * 100.0 / limit).clamp(0.0, 100.0)
    }
}

fn bar_line(
    prefix: &'static str,
    percent: f64,
    width: usize,
    filled_style: Style,
) -> Line<'static> {
    let (filled, empty) = bar_segments(percent, width);
    Line::from(vec![
        Span::styled(prefix, theme::dim()),
        Span::styled(filled, filled_style),
        Span::styled(empty, theme::border()),
    ])
}

fn bar_segments(percent: f64, width: usize) -> (String, String) {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    ("█".repeat(filled), "·".repeat(width - filled))
}

fn title(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_percent_does_not_render_as_full_bar() {
        let (filled, empty) = bar_segments(1.0, 12);

        assert!(filled.chars().count() <= 1);
        assert!(empty.chars().count() >= 11);
    }

    #[test]
    fn full_percent_renders_full_bar() {
        let (filled, empty) = bar_segments(100.0, 12);

        assert_eq!(filled.chars().count(), 12);
        assert!(empty.is_empty());
    }
}
