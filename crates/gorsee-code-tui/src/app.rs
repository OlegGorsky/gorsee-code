use gorsee_code_ui_state::WorkspaceState;

use crate::{
    actions::AppIntent,
    attachment::Attachment,
    center_panel::CenterPanel,
    completion::{completion_for, CompletionItem, CompletionKind, CompletionMenu},
    editor::EditorBuffer,
    model_picker::ModelChoice,
    navigation::{FocusPane, MENU_ITEMS},
    panel_items::PanelItem,
    project::ProjectTree,
    session_picker::SessionItem,
    KeyAction,
};

#[derive(Debug, Default)]
pub struct WorkspaceApp {
    pub(crate) input: String,
    pub(crate) input_cursor: usize,
    pub(crate) status: Option<String>,
    pub(crate) output: Option<String>,
    pub(crate) center_panel: CenterPanel,
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) project: ProjectTree,
    pub(crate) editor: Option<EditorBuffer>,
    pub(crate) completion: Option<CompletionMenu>,
    pub(crate) focus: FocusPane,
    pub(crate) selected_menu: usize,
    pub(crate) spinner_tick: u64,
    pub(crate) sessions: Vec<SessionItem>,
    pub(crate) selected_session: usize,
    pub(crate) active_session_id: Option<String>,
    pub(crate) models: Vec<ModelChoice>,
    pub(crate) selected_model: usize,
    pub(crate) panel_items: Vec<PanelItem>,
    pub(crate) selected_panel_item: usize,
    pub(crate) selection_anchor: Option<(u16, u16)>,
    pub(crate) selection_cursor: Option<(u16, u16)>,
    pub(crate) selection_range: Option<((u16, u16), (u16, u16))>,
    pub(crate) center_scroll: usize,
    pub(crate) confirm_close_editor: bool,
    pub(crate) pending_restore_input: Option<String>,
}

impl WorkspaceApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn input_cursor(&self) -> usize {
        self.input_cursor
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    pub fn center_panel(&self) -> CenterPanel {
        self.center_panel
    }

    pub fn focus_pane(&self) -> FocusPane {
        self.focus
    }

    pub fn selected_menu_label(&self) -> &'static str {
        MENU_ITEMS
            .get(self.selected_menu)
            .map(|item| item.label)
            .unwrap_or("Лента")
    }

    pub fn selected_menu_index(&self) -> usize {
        self.selected_menu
    }

    pub fn spinner_tick(&self) -> u64 {
        self.spinner_tick
    }

    pub fn session_items(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|item| item.label().to_string())
            .collect()
    }

    pub fn active_session_id(&self) -> Option<&str> {
        self.active_session_id.as_deref()
    }

    pub fn model_selected_agent(&self) -> Option<&str> {
        self.models.get(self.selected_model).map(ModelChoice::agent)
    }

    pub fn model_selected_model(&self) -> Option<&str> {
        self.models.get(self.selected_model).map(ModelChoice::model)
    }

    pub(crate) fn panel_items(&self) -> &[PanelItem] {
        &self.panel_items
    }

    pub(crate) fn selected_panel_item(&self) -> usize {
        self.selected_panel_item
    }

    pub fn advance_spinner(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    pub fn completion_kind(&self) -> Option<CompletionKind> {
        self.completion.as_ref().map(CompletionMenu::kind)
    }

    pub fn completion_items(&self) -> &[CompletionItem] {
        self.completion
            .as_ref()
            .map(CompletionMenu::items)
            .unwrap_or(&[])
    }

    pub fn completion_selected(&self) -> usize {
        self.completion
            .as_ref()
            .map(CompletionMenu::selected)
            .unwrap_or(0)
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    pub fn set_output(&mut self, output: impl Into<String>) {
        self.output = Some(output.into());
        self.center_scroll = 0;
    }

    pub fn clear_output(&mut self) {
        self.output = None;
        self.center_scroll = 0;
    }

    pub(crate) fn set_panel_items(&mut self, items: Vec<PanelItem>) {
        self.panel_items = items;
        self.selected_panel_item = 0;
    }

    pub fn handle_action(&mut self, action: KeyAction, state: &WorkspaceState) -> AppIntent {
        match action {
            KeyAction::Insert(value) => self.insert(value),
            KeyAction::Backspace => self.backspace(),
            KeyAction::MoveLeft => self.move_left(),
            KeyAction::MoveRight => self.move_right(),
            KeyAction::MoveSelectionUp => self.move_selection_up(),
            KeyAction::MoveSelectionDown => self.move_selection_down(),
            KeyAction::ScrollUp => self.scroll_editor_up(),
            KeyAction::ScrollDown => self.scroll_editor_down(),
            KeyAction::Newline => self.insert('\n'),
            KeyAction::AcceptCompletion => self.accept_completion(),
            KeyAction::FocusNext => self.focus_next(),
            KeyAction::Save => self.save_editor(),
            KeyAction::CloseEditor => self.close_editor(),
            KeyAction::Submit => self.submit(state),
            KeyAction::Approve => self.approval_intent(state, AppIntent::Approve),
            KeyAction::Deny => self.approval_intent(state, AppIntent::Deny),
            KeyAction::Pause => self.session_intent(state, AppIntent::Pause),
            KeyAction::Resume => self.session_intent(state, AppIntent::Resume),
            KeyAction::Quit => AppIntent::Quit,
            KeyAction::Ignore => AppIntent::None,
        }
    }

    pub(crate) fn refresh_completion(&mut self) {
        if self.editor.is_some() {
            self.completion = None;
            return;
        }
        self.completion = completion_for(&self.input, self.input_cursor, self.project.files());
    }

    pub(crate) fn restore_prompt(&mut self, input: String) {
        self.input = input;
        self.input_cursor = self.input.len();
        self.refresh_completion();
    }

    pub(crate) fn remember_prompt_for_restore(&mut self, input: String) {
        self.pending_restore_input = Some(input);
    }

    pub(crate) fn restore_pending_prompt(&mut self) {
        if let Some(input) = self.pending_restore_input.take() {
            self.restore_prompt(input);
        }
    }

    pub(crate) fn clear_pending_prompt(&mut self) {
        self.pending_restore_input = None;
    }

    pub(crate) fn clear_attachments(&mut self) {
        self.attachments.clear();
    }

    pub(crate) fn scroll_center_up(&mut self) {
        self.center_scroll = self.center_scroll.saturating_sub(1);
    }

    pub(crate) fn scroll_center_down(&mut self) {
        self.center_scroll = self.center_scroll.saturating_add(1);
    }

    pub fn center_scroll(&self) -> usize {
        self.center_scroll
    }

    pub(crate) fn center_selection_range(&self) -> Option<((u16, u16), (u16, u16))> {
        self.selection_range
            .or_else(|| Some((self.selection_anchor?, self.selection_cursor?)))
    }

    pub(crate) fn completion_visible_start(&self) -> usize {
        let selected = self.completion_selected();
        if selected >= 8 {
            selected + 1 - 8
        } else {
            0
        }
    }

    pub(crate) fn composer_rows(&self) -> u16 {
        let input_rows = self.input.chars().filter(|ch| *ch == '\n').count() as u16 + 1;
        let completion_rows = if self.completion_items().is_empty() {
            0
        } else {
            self.completion_items().len().min(8) as u16 + 2
        };
        let attachment_rows = if self.attachments().is_empty() {
            0
        } else {
            self.attachments().len() as u16 + 2
        };
        input_rows + completion_rows + attachment_rows
    }
}
