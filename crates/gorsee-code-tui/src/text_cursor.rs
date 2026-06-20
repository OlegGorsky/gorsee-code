use ratatui::layout::Rect;

pub(crate) fn clamp_to_boundary(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

pub(crate) fn previous_boundary(text: &str, cursor: usize) -> usize {
    let cursor = clamp_to_boundary(text, cursor);
    text[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

pub(crate) fn next_boundary(text: &str, cursor: usize) -> usize {
    let cursor = clamp_to_boundary(text, cursor);
    text[cursor..]
        .chars()
        .next()
        .map(|ch| cursor + ch.len_utf8())
        .unwrap_or(text.len())
}

pub(crate) fn cursor_for_position(text: &str, target_line: usize, target_column: usize) -> usize {
    let mut byte = 0;
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index == target_line {
            return byte + byte_index_for_column(line, target_column);
        }
        byte += line.len() + 1;
    }
    text.len()
}

pub(crate) fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn byte_index_for_column(line: &str, target_column: usize) -> usize {
    line.char_indices()
        .nth(target_column)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}
