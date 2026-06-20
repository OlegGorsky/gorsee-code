use crossterm::event::MouseEvent;
use gorsee_code_ui_state::{EventView, WorkspaceState};
use ratatui::layout::Rect;

use crate::WorkspaceApp;

pub(crate) fn selected_center_text(
    state: &WorkspaceState,
    app: &WorkspaceApp,
    center: Rect,
    end: (u16, u16),
) -> String {
    let Some((start_column, start_row)) = app.selection_anchor else {
        return String::new();
    };
    selected_text(
        &copyable_center_lines(state, app),
        center,
        app.center_scroll(),
        (start_column, start_row),
        end,
    )
}

pub(crate) fn mouse_point(mouse: MouseEvent) -> (u16, u16) {
    (mouse.column, mouse.row)
}

fn copyable_center_lines(state: &WorkspaceState, app: &WorkspaceApp) -> Vec<String> {
    if let Some(editor) = app.editor() {
        return editor.text().lines().map(ToOwned::to_owned).collect();
    }
    if app.output().is_some() {
        return app
            .output()
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect();
    }
    let mut lines = vec![
        format!("Gorsee Code - Лента {}", state.session.id),
        String::new(),
    ];
    for event in &state.timeline {
        lines.push(event_line(event));
    }
    lines
}

fn event_line(event: &EventView) -> String {
    let actor = event
        .agent_id
        .as_deref()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| event.kind.clone());
    format!("#{:04} {} · {}", event.sequence, actor, event.summary)
}

fn selected_text(
    lines: &[String],
    center: Rect,
    scroll: usize,
    start: (u16, u16),
    end: (u16, u16),
) -> String {
    let start_row = scroll + row_index(center, start.1).min(row_index(center, end.1));
    let end_row = scroll + row_index(center, start.1).max(row_index(center, end.1));
    let left = column_index(center, start.0).min(column_index(center, end.0));
    let right = column_index(center, start.0).max(column_index(center, end.0));
    let mut selected = lines
        .iter()
        .skip(start_row)
        .take(end_row.saturating_sub(start_row) + 1)
        .map(|line| slice_columns(line, left, right))
        .collect::<Vec<_>>()
        .join("\n");
    if selected.ends_with('\n') {
        selected.pop();
    }
    selected
}

fn row_index(area: Rect, row: u16) -> usize {
    row.saturating_sub(area.y + 1) as usize
}

fn column_index(area: Rect, column: u16) -> usize {
    column.saturating_sub(area.x + 1) as usize
}

fn slice_columns(line: &str, left: usize, right: usize) -> String {
    let start = byte_index_for_column(line, left);
    let end = byte_index_for_column(line, right.saturating_add(1));
    line[start..end].to_string()
}

fn byte_index_for_column(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}
