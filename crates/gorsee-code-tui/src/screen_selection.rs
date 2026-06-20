use ratatui::{
    layout::Rect,
    text::{Line, Span},
};

use crate::{theme, WorkspaceApp};

pub(crate) fn apply_center_selection(
    lines: Vec<Line<'static>>,
    area: Rect,
    app: &WorkspaceApp,
) -> Vec<Line<'static>> {
    let Some((start, end)) = app.center_selection_range() else {
        return lines;
    };
    let top = row_index(area, start.1).min(row_index(area, end.1));
    let bottom = row_index(area, start.1).max(row_index(area, end.1));
    let left = column_index(area, start.0).min(column_index(area, end.0));
    let right = column_index(area, start.0).max(column_index(area, end.0));

    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if index < top || index > bottom {
                line
            } else {
                highlight_line(line, left, right)
            }
        })
        .collect()
}

fn highlight_line(line: Line<'static>, left: usize, right: usize) -> Line<'static> {
    let text = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let start = byte_index_for_column(&text, left);
    let end = byte_index_for_column(&text, right.saturating_add(1));
    if start >= end {
        return line;
    }
    Line::from(vec![
        Span::styled(text[..start].to_string(), theme::normal()),
        Span::styled(text[start..end].to_string(), theme::selection()),
        Span::styled(text[end..].to_string(), theme::normal()),
    ])
}

fn row_index(area: Rect, row: u16) -> usize {
    row.saturating_sub(area.y + 1) as usize
}

fn column_index(area: Rect, column: u16) -> usize {
    column.saturating_sub(area.x + 1) as usize
}

fn byte_index_for_column(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}
