use gorsee_code_ui_state::WorkspaceState;

use crate::{
    actions::AppIntent,
    attachment::attachment_from_paste,
    center_panel::CenterPanel,
    navigation::{FocusPane, MENU_ITEMS},
    text_cursor::{clamp_to_boundary, next_boundary, previous_boundary},
    KeyAction, WorkspaceApp,
};

impl WorkspaceApp {
    pub fn paste_text(&mut self, text: &str, state: &WorkspaceState) {
        if self.editor.is_none() {
            if let Some(attachment) = attachment_from_paste(text) {
                let token = format!("@{}", attachment.label());
                self.attachments.push(attachment);
                self.insert_token(&token);
                self.set_status("вложение добавлено");
                return;
            }
        }
        for value in text.chars() {
            match value {
                '\r' => {}
                '\n' => {
                    self.handle_action(KeyAction::Newline, state);
                }
                value => {
                    self.handle_action(KeyAction::Insert(value), state);
                }
            }
        }
    }

    pub(crate) fn insert(&mut self, value: char) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.insert(value);
            self.confirm_close_editor = false;
            self.set_status("file modified");
            return AppIntent::None;
        }
        self.input_cursor = clamp_to_boundary(&self.input, self.input_cursor);
        self.input.insert(self.input_cursor, value);
        self.input_cursor += value.len_utf8();
        self.refresh_completion();
        AppIntent::None
    }

    pub(crate) fn backspace(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.backspace();
            self.confirm_close_editor = false;
            self.set_status("file modified");
            return AppIntent::None;
        }
        self.input_cursor = clamp_to_boundary(&self.input, self.input_cursor);
        if self.input_cursor == 0 {
            return AppIntent::None;
        }
        let previous = previous_boundary(&self.input, self.input_cursor);
        self.input.replace_range(previous..self.input_cursor, "");
        self.input_cursor = previous;
        self.refresh_completion();
        AppIntent::None
    }

    pub(crate) fn move_left(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.move_left();
            return AppIntent::None;
        }
        if self.panel_navigation_active() {
            self.select_model_previous();
            return AppIntent::None;
        }
        if self.input.is_empty() && self.completion.is_none() && self.focus == FocusPane::Files {
            if let Err(error) = self.choose_parent_folder() {
                self.set_status(format!("folder failed: {error}"));
            }
            return AppIntent::None;
        }
        self.input_cursor = previous_boundary(&self.input, self.input_cursor);
        self.refresh_completion();
        AppIntent::None
    }

    pub(crate) fn move_right(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.move_right();
            return AppIntent::None;
        }
        if self.panel_navigation_active() {
            self.select_model_next();
            return AppIntent::None;
        }
        if self.input.is_empty() && self.completion.is_none() && self.focus == FocusPane::Files {
            if let Err(error) = self.choose_selected_folder() {
                self.set_status(format!("folder failed: {error}"));
            }
            return AppIntent::None;
        }
        self.input_cursor = next_boundary(&self.input, self.input_cursor);
        self.refresh_completion();
        AppIntent::None
    }

    pub(crate) fn move_selection_up(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.scroll_up();
            return AppIntent::None;
        }
        if let Some(completion) = self.completion.as_mut() {
            completion.select_previous();
        } else if self.panel_navigation_active() {
            self.select_panel_previous();
        } else if self.focus == FocusPane::Menu {
            self.selected_menu = self.selected_menu.saturating_sub(1);
        } else {
            self.project.select_previous();
            self.project.ensure_selected_visible(12);
        }
        AppIntent::None
    }

    pub(crate) fn move_selection_down(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.scroll_down();
            return AppIntent::None;
        }
        if let Some(completion) = self.completion.as_mut() {
            completion.select_next();
        } else if self.panel_navigation_active() {
            self.select_panel_next();
        } else if self.focus == FocusPane::Menu {
            if self.selected_menu + 1 < MENU_ITEMS.len() {
                self.selected_menu += 1;
            }
        } else {
            self.project.select_next();
            self.project.ensure_selected_visible(12);
        }
        AppIntent::None
    }

    fn panel_navigation_active(&self) -> bool {
        self.input.is_empty()
            && self.completion.is_none()
            && self.focus == FocusPane::Menu
            && matches!(
                self.center_panel,
                CenterPanel::Sessions | CenterPanel::Models
            )
    }

    fn select_panel_previous(&mut self) {
        match self.center_panel {
            CenterPanel::Sessions => {
                self.selected_session = self.selected_session.saturating_sub(1);
            }
            CenterPanel::Models => {
                self.selected_model = self.selected_model.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn select_panel_next(&mut self) {
        match self.center_panel {
            CenterPanel::Sessions => {
                if self.selected_session + 1 < self.sessions.len() {
                    self.selected_session += 1;
                }
            }
            CenterPanel::Models => {
                if self.selected_model + 1 < self.models.len() {
                    self.selected_model += 1;
                }
            }
            _ => {}
        }
    }

    fn select_model_previous(&mut self) {
        if let Some(choice) = self.models.get_mut(self.selected_model) {
            choice.select_previous();
        }
    }

    fn select_model_next(&mut self) {
        if let Some(choice) = self.models.get_mut(self.selected_model) {
            choice.select_next();
        }
    }

    pub(crate) fn scroll_editor_up(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.scroll_up();
        } else {
            self.scroll_center_up();
        }
        AppIntent::None
    }

    pub(crate) fn scroll_editor_down(&mut self) -> AppIntent {
        if let Some(editor) = self.editor.as_mut() {
            editor.scroll_down();
        } else {
            self.scroll_center_down();
        }
        AppIntent::None
    }

    pub(crate) fn accept_completion(&mut self) -> AppIntent {
        let Some(completion) = self.completion.take() else {
            return AppIntent::None;
        };
        completion.accept(&mut self.input, &mut self.input_cursor);
        self.refresh_completion();
        AppIntent::None
    }

    pub(crate) fn focus_next(&mut self) -> AppIntent {
        if self.editor.is_some() {
            return AppIntent::None;
        }
        self.focus = match self.focus {
            FocusPane::Menu => FocusPane::Files,
            FocusPane::Files => FocusPane::Menu,
        };
        self.set_status(match self.focus {
            FocusPane::Menu => "фокус: меню",
            FocusPane::Files => "фокус: файлы",
        });
        AppIntent::None
    }

    fn insert_token(&mut self, token: &str) {
        if !self.input.is_empty()
            && self
                .input
                .get(..self.input_cursor)
                .and_then(|before| before.chars().last())
                .map(|ch| !ch.is_whitespace())
                .unwrap_or(false)
        {
            self.input.insert(self.input_cursor, ' ');
            self.input_cursor += 1;
        }
        self.input.insert_str(self.input_cursor, token);
        self.input_cursor += token.len();
        self.refresh_completion();
    }
}
