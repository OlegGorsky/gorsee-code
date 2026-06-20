use std::path::Path;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use gorsee_code_ui_state::{EventView, WorkspaceState};
use ratatui::layout::Rect;

use crate::{
    center_panel::CenterPanel,
    layout::screen_layout,
    navigation::MENU_ITEMS,
    text_cursor::{contains, cursor_for_position},
    AppIntent, WorkspaceApp,
};

impl WorkspaceApp {
    pub fn project_row_for_path(&self, path: &Path) -> Option<u16> {
        self.project
            .entries()
            .iter()
            .position(|entry| entry.path() == path)
            .and_then(|index| {
                index
                    .checked_sub(self.project.scroll())
                    .map(|visible| project_entry_line() + visible as u16 + 1)
            })
    }

    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        state: &WorkspaceState,
    ) -> AppIntent {
        let layout = screen_layout(area, self.composer_rows());
        match mouse.kind {
            MouseEventKind::ScrollDown => return self.handle_scroll(mouse, layout, true),
            MouseEventKind::ScrollUp => return self.handle_scroll(mouse, layout, false),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(intent) =
                    self.handle_center_click(mouse.column, mouse.row, layout.center)
                {
                    return intent;
                }
                if contains(layout.center, mouse.column, mouse.row) {
                    self.selection_anchor = Some((mouse.column, mouse.row));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                return self.handle_drag_copy(mouse, layout.center, state);
            }
            MouseEventKind::Drag(MouseButton::Left) => {}
            _ => return AppIntent::None,
        }
        if contains(layout.left, mouse.column, mouse.row) {
            return self.handle_sidebar_mouse(mouse.row, layout.left, state);
        } else if contains(layout.composer, mouse.column, mouse.row) {
            if self.handle_completion_mouse(mouse.row, layout.composer) {
                return AppIntent::None;
            }
            self.place_input_cursor(mouse.column, mouse.row, layout.composer);
        }
        AppIntent::None
    }

    fn handle_scroll(
        &mut self,
        mouse: MouseEvent,
        layout: crate::layout::ScreenLayout,
        down: bool,
    ) -> AppIntent {
        if contains(layout.center, mouse.column, mouse.row) {
            if down {
                if self.editor().is_some() {
                    self.scroll_editor_down();
                } else {
                    self.scroll_center_down();
                }
            } else if self.editor().is_some() {
                self.scroll_editor_up();
            } else {
                self.scroll_center_up();
            }
        } else if contains(layout.left, mouse.column, mouse.row) {
            if down {
                self.project.select_next();
            } else {
                self.project.select_previous();
            }
            self.project.ensure_selected_visible(12);
        }
        AppIntent::None
    }

    fn handle_drag_copy(
        &mut self,
        mouse: MouseEvent,
        center: Rect,
        state: &WorkspaceState,
    ) -> AppIntent {
        let Some((start_column, start_row)) = self.selection_anchor else {
            return AppIntent::None;
        };
        if !contains(center, mouse.column, mouse.row) {
            self.selection_anchor = None;
            return AppIntent::None;
        }
        let text = selected_center_text(
            &copyable_center_lines(state, self),
            center,
            self.center_scroll(),
            start_column,
            start_row,
            mouse.column,
            mouse.row,
        );
        if text.trim().is_empty() {
            self.selection_anchor = None;
            return AppIntent::None;
        }
        self.set_status("Скопировано!");
        self.selection_anchor = None;
        AppIntent::Copy(text)
    }

    fn handle_center_click(&mut self, column: u16, row: u16, center: Rect) -> Option<AppIntent> {
        if !contains(center, column, row) {
            return None;
        }
        if row < center.y + 4 {
            return None;
        }
        let index = usize::from(row.saturating_sub(center.y + 4));
        match self.center_panel {
            CenterPanel::Sessions if index < self.sessions.len() => {
                self.selected_session = index;
                self.activate_center_selection()
            }
            CenterPanel::Models if index < self.models.len() => {
                self.selected_model = index;
                self.activate_center_selection()
            }
            _ => None,
        }
    }

    fn handle_sidebar_mouse(&mut self, row: u16, left: Rect, state: &WorkspaceState) -> AppIntent {
        let content_row = row.saturating_sub(left.y + 1);
        if let Some(intent) = self.handle_menu_mouse(content_row, state) {
            return intent;
        }
        self.handle_project_mouse(row, left);
        AppIntent::None
    }

    fn handle_menu_mouse(&mut self, content_row: u16, state: &WorkspaceState) -> Option<AppIntent> {
        if content_row == 0 || usize::from(content_row) > MENU_ITEMS.len() {
            return None;
        }
        self.selected_menu = usize::from(content_row - 1);
        Some(self.activate_menu(state))
    }

    fn handle_project_mouse(&mut self, row: u16, left: Rect) {
        let first_row = left.y + project_entry_line() + 1;
        if row < first_row {
            return;
        }
        let index = self.project.scroll() + usize::from(row - first_row);
        let Some(entry) = self.project.entry(index).cloned() else {
            return;
        };
        if entry.is_dir() {
            if let Err(error) = self.project.toggle_dir(entry.path()) {
                self.set_status(format!("project error: {error}"));
            }
        } else if let Err(error) = self.open_project_file(entry.path()) {
            self.set_status(format!("open failed: {error}"));
        }
    }

    fn handle_completion_mouse(&mut self, row: u16, composer: Rect) -> bool {
        let visible_start = self.completion_visible_start();
        let Some(completion) = self.completion.as_mut() else {
            return false;
        };
        let input_rows = self.input.chars().filter(|ch| *ch == '\n').count() as u16 + 1;
        let first_item_row = composer.y + 1 + input_rows + 2;
        if row < first_item_row {
            return false;
        }
        let index = visible_start + usize::from(row - first_item_row);
        if index >= completion.items().len() {
            return false;
        }
        completion.select(index);
        self.accept_completion();
        true
    }

    fn place_input_cursor(&mut self, column: u16, row: u16, composer: Rect) {
        if self.editor.is_some() {
            return;
        }
        let x = column.saturating_sub(composer.x + 1) as usize;
        let y = row.saturating_sub(composer.y + 1) as usize;
        self.input_cursor = cursor_for_position(&self.input, y, x);
        self.refresh_completion();
    }
}

fn project_entry_line() -> u16 {
    11
}

fn copyable_center_lines(state: &WorkspaceState, app: &WorkspaceApp) -> Vec<String> {
    if let Some(editor) = app.editor() {
        return editor.text().lines().map(ToOwned::to_owned).collect();
    }
    if app.output().is_some() && app.center_panel() != CenterPanel::Timeline {
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
        lines.push(event_title_line(event));
        lines.push(event_summary_line(event));
    }
    if let Some(output) = app.output() {
        lines.extend(output.lines().map(ToOwned::to_owned));
    }
    lines
}

fn event_title_line(event: &EventView) -> String {
    let agent = event.agent_id.as_deref().unwrap_or("workspace");
    format!("#{:04} {} {}", event.sequence, event.kind, agent)
}

fn event_summary_line(event: &EventView) -> String {
    format!("       │ {}", event.summary)
}

fn selected_center_text(
    lines: &[String],
    center: Rect,
    scroll: usize,
    start_column: u16,
    start_row: u16,
    end_column: u16,
    end_row: u16,
) -> String {
    let start = scroll + row_index(center, start_row).min(row_index(center, end_row));
    let end = scroll + row_index(center, start_row).max(row_index(center, end_row));
    let left = column_index(center, start_column).min(column_index(center, end_column));
    let right = column_index(center, start_column).max(column_index(center, end_column));
    let mut selected = lines
        .iter()
        .skip(start)
        .take(end.saturating_sub(start) + 1)
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
