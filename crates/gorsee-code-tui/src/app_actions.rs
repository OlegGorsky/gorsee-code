use gorsee_code_ui_state::WorkspaceState;

use crate::{
    actions::AppIntent,
    center_panel::CenterPanel,
    model_picker::choices_from_state,
    navigation::{FocusPane, MenuPanel, MENU_ITEMS},
    parse_command, CommandAction, WorkspaceApp,
};

impl WorkspaceApp {
    pub(crate) fn save_editor(&mut self) -> AppIntent {
        let Some(editor) = self.editor.as_mut() else {
            self.set_status("no file open");
            return AppIntent::None;
        };
        match editor.save() {
            Ok(()) => {
                self.confirm_close_editor = false;
                self.set_status("file saved");
            }
            Err(error) => self.set_status(format!("save failed: {error}")),
        }
        AppIntent::None
    }

    pub(crate) fn close_editor(&mut self) -> AppIntent {
        if self
            .editor
            .as_ref()
            .map(|editor| editor.is_dirty())
            .unwrap_or(false)
            && !self.confirm_close_editor
        {
            self.confirm_close_editor = true;
            self.set_status("файл изменен: Ctrl+W еще раз закрыть без сохранения");
            return AppIntent::None;
        }
        self.editor = None;
        self.confirm_close_editor = false;
        self.set_status("editor closed");
        AppIntent::None
    }

    pub(crate) fn submit(&mut self, state: &WorkspaceState) -> AppIntent {
        if self.editor.is_some() {
            return self.insert('\n');
        }
        if self
            .completion
            .as_ref()
            .map(|completion| completion.would_change_input(&self.input))
            .unwrap_or(false)
        {
            return self.accept_completion();
        }
        let objective = self.input.trim().to_string();
        if objective.is_empty() {
            if let Some(intent) = self.activate_center_selection() {
                return intent;
            }
            return self.activate_selection(state);
        }
        let original_input = self.input.clone();
        self.input.clear();
        self.input_cursor = 0;
        self.completion = None;
        self.remember_prompt_for_restore(original_input);
        if objective.starts_with('/') {
            return self.run_command_input(objective, state);
        }
        self.clear_output();
        self.center_panel = CenterPanel::Timeline;
        let objective = self.objective_with_attachments(objective);
        self.set_output(format!(
            "Пользователь\n{}\n\nЗапускаю агентную сессию...",
            objective
        ));
        self.set_status("running");
        AppIntent::Submit(objective)
    }

    fn objective_with_attachments(&mut self, mut objective: String) -> String {
        if self.attachments.is_empty() {
            return objective;
        }
        objective.push_str("\n\nВложения:");
        for attachment in &self.attachments {
            objective.push_str(&format!(
                "\n- {}: {}",
                attachment.label(),
                attachment.path().display()
            ));
        }
        objective
    }

    pub(crate) fn approval_intent(
        &mut self,
        state: &WorkspaceState,
        build: fn(String) -> AppIntent,
    ) -> AppIntent {
        match latest_approval_id(state) {
            Some(id) => build(id),
            None => {
                self.set_status("no pending approvals");
                AppIntent::None
            }
        }
    }

    pub(crate) fn session_intent(
        &mut self,
        state: &WorkspaceState,
        build: fn(String) -> AppIntent,
    ) -> AppIntent {
        match active_session_id(state) {
            Some(id) => build(id),
            None => {
                self.set_status("no active session");
                AppIntent::None
            }
        }
    }

    pub(crate) fn run_command_input(&mut self, input: String, state: &WorkspaceState) -> AppIntent {
        match parse_command(&input, state) {
            CommandAction::Display(output) => {
                self.clear_pending_prompt();
                self.center_panel = CenterPanel::Timeline;
                self.set_status("ready");
                self.set_output(output);
                AppIntent::None
            }
            CommandAction::External(line) => {
                self.clear_output();
                self.center_panel = panel_for_command(&line);
                self.completion = None;
                self.set_status("working");
                AppIntent::Command(line)
            }
            CommandAction::Approve(id) => AppIntent::Approve(id),
            CommandAction::Deny(id) => AppIntent::Deny(id),
            CommandAction::Pause(id) => AppIntent::Pause(id),
            CommandAction::Resume(id) => AppIntent::Resume(id),
            CommandAction::Quit => {
                self.clear_pending_prompt();
                AppIntent::Quit
            }
        }
    }

    fn activate_selection(&mut self, state: &WorkspaceState) -> AppIntent {
        if self.focus == FocusPane::Menu {
            return self.activate_menu(state);
        }
        let Some(entry) = self.project.entry(self.project.selected()).cloned() else {
            self.set_status("ready");
            return AppIntent::None;
        };
        if entry.is_dir() {
            let path = entry.path().to_path_buf();
            match self.project.toggle_dir(&path) {
                Ok(()) => self.project.ensure_selected_visible(12),
                Err(error) => self.set_status(format!("project toggle failed: {error}")),
            }
            return AppIntent::None;
        }
        if let Err(error) = self.open_project_file(entry.path()) {
            self.set_status(format!("open failed: {error}"));
        }
        AppIntent::None
    }

    pub(crate) fn activate_menu(&mut self, state: &WorkspaceState) -> AppIntent {
        let item = MENU_ITEMS.get(self.selected_menu).unwrap_or(&MENU_ITEMS[0]);
        match item.panel {
            MenuPanel::Timeline => self.run_command_input("/timeline".into(), state),
            MenuPanel::Diff => self.run_command_input("/diff".into(), state),
            MenuPanel::Sessions => self.open_sessions_panel(),
            MenuPanel::Models => self.open_models_panel(state),
            MenuPanel::Instructions => self.run_command_input("/instructions".into(), state),
            MenuPanel::Skills => self.run_command_input("/skills".into(), state),
            MenuPanel::Mcp => self.run_command_input("/mcp".into(), state),
            MenuPanel::Limits => self.run_command_input("/limits".into(), state),
        }
    }

    fn open_sessions_panel(&mut self) -> AppIntent {
        let Some(root) = self.working_folder().map(std::path::Path::to_path_buf) else {
            self.set_status("project root is not ready");
            return AppIntent::None;
        };
        match crate::session_picker::session_ids(&root) {
            Ok(sessions) => {
                self.sessions = sessions;
                self.selected_session = selected_index(&self.sessions, self.active_session_id());
                self.center_panel = CenterPanel::Sessions;
                self.clear_output();
                self.set_status("сессии");
            }
            Err(error) => self.set_status(format!("sessions failed: {error}")),
        }
        AppIntent::None
    }

    fn open_models_panel(&mut self, state: &WorkspaceState) -> AppIntent {
        self.models = choices_from_state(state);
        self.selected_model = self.selected_model.min(self.models.len().saturating_sub(1));
        self.center_panel = CenterPanel::Models;
        self.clear_output();
        self.set_status("модели");
        AppIntent::None
    }

    pub(crate) fn activate_center_selection(&mut self) -> Option<AppIntent> {
        match self.center_panel {
            CenterPanel::Sessions => {
                let id = self.sessions.get(self.selected_session)?.clone();
                self.active_session_id = Some(id.clone());
                self.center_panel = CenterPanel::Timeline;
                self.clear_output();
                self.set_status(format!("сессия: {id}"));
                Some(AppIntent::None)
            }
            CenterPanel::Models => {
                let choice = self.models.get(self.selected_model)?;
                Some(AppIntent::Command(format!(
                    "models set --agent {} --model {}",
                    choice.agent(),
                    choice.model()
                )))
            }
            _ => None,
        }
    }
}

fn selected_index(items: &[String], active: Option<&str>) -> usize {
    active
        .and_then(|id| items.iter().position(|item| item == id))
        .unwrap_or(0)
}

fn panel_for_command(line: &str) -> CenterPanel {
    match line.split_whitespace().next() {
        Some("diff") => CenterPanel::Diff,
        Some("sessions") => CenterPanel::Sessions,
        Some("models") => CenterPanel::Models,
        Some("instructions") => CenterPanel::Instructions,
        Some("skills") => CenterPanel::Skills,
        Some("mcp") => CenterPanel::Mcp,
        Some("limits") => CenterPanel::Limits,
        Some("terminal") => CenterPanel::Terminal,
        _ => CenterPanel::Timeline,
    }
}

fn latest_approval_id(state: &WorkspaceState) -> Option<String> {
    state.approvals.first().map(|approval| approval.id.clone())
}

fn active_session_id(state: &WorkspaceState) -> Option<String> {
    let id = state.session.id.trim();
    (!id.is_empty() && id != "workspace").then(|| id.to_string())
}
