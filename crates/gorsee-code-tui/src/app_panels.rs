use gorsee_code_ui_state::WorkspaceState;

use crate::{
    actions::AppIntent,
    center_panel::CenterPanel,
    model_picker::choices_from_state,
    navigation::FocusPane,
    panel_items::{
        ensure_target, instruction_items, limit_items, mcp_items, project_items, skill_items,
        PanelItemTarget,
    },
    WorkspaceApp,
};

impl WorkspaceApp {
    pub(crate) fn open_timeline_panel(&mut self) -> AppIntent {
        self.center_panel = CenterPanel::Timeline;
        self.focus = FocusPane::Menu;
        self.panel_items.clear();
        self.clear_output();
        self.set_status("лента");
        AppIntent::None
    }

    pub(crate) fn open_project_panel(&mut self) -> AppIntent {
        let Some(root) = self.working_folder().map(std::path::Path::to_path_buf) else {
            self.set_status("project root is not ready");
            return AppIntent::None;
        };
        self.set_panel_items(project_items(&root));
        self.center_panel = CenterPanel::Project;
        self.focus = FocusPane::Menu;
        self.clear_output();
        self.set_status("проект");
        AppIntent::None
    }

    pub(crate) fn open_sessions_panel(&mut self) -> AppIntent {
        let Some(root) = self.working_folder().map(std::path::Path::to_path_buf) else {
            self.set_status("project root is not ready");
            return AppIntent::None;
        };
        match crate::session_picker::session_items(&root) {
            Ok(sessions) => {
                self.sessions = sessions;
                self.selected_session = selected_index(&self.sessions, self.active_session_id());
                self.center_panel = CenterPanel::Sessions;
                self.focus = FocusPane::Menu;
                self.panel_items.clear();
                self.clear_output();
                self.set_status("сессии");
            }
            Err(error) => self.set_status(format!("sessions failed: {error}")),
        }
        AppIntent::None
    }

    pub(crate) fn open_models_panel(&mut self, state: &WorkspaceState) -> AppIntent {
        self.models = choices_from_state(state);
        self.selected_model = self.selected_model.min(self.models.len().saturating_sub(1));
        self.center_panel = CenterPanel::Models;
        self.focus = FocusPane::Menu;
        self.panel_items.clear();
        self.clear_output();
        self.set_status("модели");
        AppIntent::None
    }

    pub(crate) fn open_item_panel(&mut self, panel: CenterPanel) -> AppIntent {
        let Some(root) = self.working_folder().map(std::path::Path::to_path_buf) else {
            self.set_status("project root is not ready");
            return AppIntent::None;
        };
        let items = match panel {
            CenterPanel::Project => project_items(&root),
            CenterPanel::Instructions => instruction_items(&root),
            CenterPanel::Skills => skill_items(&root),
            CenterPanel::Mcp => mcp_items(&root),
            CenterPanel::Limits => limit_items(),
            _ => Vec::new(),
        };
        self.set_panel_items(items);
        self.center_panel = panel;
        self.focus = FocusPane::Menu;
        self.clear_output();
        self.set_status(panel_status(panel));
        AppIntent::None
    }

    pub(crate) fn activate_center_selection(&mut self) -> Option<AppIntent> {
        match self.center_panel {
            CenterPanel::Project => select_project_item(self),
            CenterPanel::Sessions => select_session(self),
            CenterPanel::Models => select_model(self),
            CenterPanel::Instructions | CenterPanel::Skills | CenterPanel::Mcp => {
                open_selected_item(self)
            }
            CenterPanel::Limits => Some(AppIntent::Command("limits watch --once".into())),
            _ => None,
        }
    }
}

fn select_session(app: &mut WorkspaceApp) -> Option<AppIntent> {
    let item = app.sessions.get(app.selected_session)?;
    let Some(id) = item.id().map(ToOwned::to_owned) else {
        app.active_session_id = None;
        app.center_panel = CenterPanel::Timeline;
        app.clear_output();
        app.set_status("новая сессия: введите задачу");
        return Some(AppIntent::None);
    };
    app.active_session_id = Some(id.clone());
    app.center_panel = CenterPanel::Timeline;
    app.clear_output();
    app.set_status(format!("сессия: {id}"));
    Some(AppIntent::None)
}

fn select_project_item(app: &mut WorkspaceApp) -> Option<AppIntent> {
    let item = app.panel_items.get(app.selected_panel_item)?;
    match item.target() {
        PanelItemTarget::ProjectPath(path) => {
            let path = path.clone();
            if let Err(error) = app.choose_working_folder(path) {
                app.set_status(format!("project failed: {error}"));
            } else {
                app.open_project_panel();
            }
        }
        _ => app.set_status("путь: введите /project <путь>"),
    }
    Some(AppIntent::None)
}

fn select_model(app: &WorkspaceApp) -> Option<AppIntent> {
    let choice = app.models.get(app.selected_model)?;
    Some(AppIntent::Command(format!(
        "models set --agent {} --model {}",
        choice.agent(),
        choice.model()
    )))
}

fn open_selected_item(app: &mut WorkspaceApp) -> Option<AppIntent> {
    let root = app.working_folder().map(std::path::Path::to_path_buf)?;
    let item = app.panel_items.get(app.selected_panel_item)?;
    if let PanelItemTarget::ProjectPath(path) = item.target() {
        let path = path.clone();
        if let Err(error) = app.choose_working_folder(path) {
            app.set_status(format!("project failed: {error}"));
        }
        return Some(AppIntent::None);
    }
    match ensure_target(&root, item.target()) {
        Ok(Some(path)) => {
            if let Err(error) = app.open_project_file(path) {
                app.set_status(format!("open failed: {error}"));
            }
        }
        Ok(None) => app.set_status("нет редактируемого файла"),
        Err(error) => app.set_status(format!("create failed: {error}")),
    }
    Some(AppIntent::None)
}

fn panel_status(panel: CenterPanel) -> &'static str {
    match panel {
        CenterPanel::Project => "проект",
        CenterPanel::Instructions => "инструкции",
        CenterPanel::Skills => "скиллы",
        CenterPanel::Mcp => "mcp",
        CenterPanel::Limits => "лимиты",
        _ => "ready",
    }
}

fn selected_index(items: &[crate::session_picker::SessionItem], active: Option<&str>) -> usize {
    active
        .and_then(|id| items.iter().position(|item| item.id() == Some(id)))
        .unwrap_or(0)
}
