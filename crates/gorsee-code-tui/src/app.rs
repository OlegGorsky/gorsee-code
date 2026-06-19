use std::sync::Arc;

use anyhow::Result;
use gorsee_code_ui_state::WorkspaceState;

use crate::{parse_command, CommandAction, KeyAction};

type Handler = Arc<dyn Fn(String) -> Result<String> + Send + Sync>;

#[derive(Clone)]
pub struct TuiHandlers {
    pub(crate) submit: Handler,
    pub(crate) approve: Handler,
    pub(crate) deny: Handler,
    pub(crate) pause: Handler,
    pub(crate) resume: Handler,
    pub(crate) command: Handler,
}

impl TuiHandlers {
    pub fn new<S, A, D, P, R, C>(
        submit: S,
        approve: A,
        deny: D,
        pause: P,
        resume: R,
        command: C,
    ) -> Self
    where
        S: Fn(String) -> Result<String> + Send + Sync + 'static,
        A: Fn(String) -> Result<String> + Send + Sync + 'static,
        D: Fn(String) -> Result<String> + Send + Sync + 'static,
        P: Fn(String) -> Result<String> + Send + Sync + 'static,
        R: Fn(String) -> Result<String> + Send + Sync + 'static,
        C: Fn(String) -> Result<String> + Send + Sync + 'static,
    {
        Self {
            submit: Arc::new(submit),
            approve: Arc::new(approve),
            deny: Arc::new(deny),
            pause: Arc::new(pause),
            resume: Arc::new(resume),
            command: Arc::new(command),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    None,
    Submit(String),
    Approve(String),
    Deny(String),
    Pause(String),
    Resume(String),
    Command(String),
    Quit,
}

#[derive(Debug, Default)]
pub struct WorkspaceApp {
    input: String,
    status: Option<String>,
    output: Option<String>,
}

impl WorkspaceApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    pub fn set_output(&mut self, output: impl Into<String>) {
        self.output = Some(output.into());
    }

    pub fn clear_output(&mut self) {
        self.output = None;
    }

    pub fn handle_action(&mut self, action: KeyAction, state: &WorkspaceState) -> AppIntent {
        match action {
            KeyAction::Insert(value) => self.insert(value),
            KeyAction::Backspace => self.backspace(),
            KeyAction::Submit => self.submit(state),
            KeyAction::Approve => self.approval_intent(state, AppIntent::Approve),
            KeyAction::Deny => self.approval_intent(state, AppIntent::Deny),
            KeyAction::Pause => self.session_intent(state, AppIntent::Pause),
            KeyAction::Resume => self.session_intent(state, AppIntent::Resume),
            KeyAction::Quit => AppIntent::Quit,
            KeyAction::Ignore => AppIntent::None,
        }
    }

    fn insert(&mut self, value: char) -> AppIntent {
        self.input.push(value);
        AppIntent::None
    }

    fn backspace(&mut self) -> AppIntent {
        self.input.pop();
        AppIntent::None
    }

    fn submit(&mut self, state: &WorkspaceState) -> AppIntent {
        let objective = self.input.trim().to_string();
        if objective.is_empty() {
            self.set_status("ready");
            return AppIntent::None;
        }
        self.input.clear();
        if objective.starts_with('/') {
            return self.command(objective, state);
        }
        self.clear_output();
        self.set_status("running");
        AppIntent::Submit(objective)
    }

    fn command(&mut self, input: String, state: &WorkspaceState) -> AppIntent {
        match parse_command(&input, state) {
            CommandAction::Display(output) => {
                self.set_status("ready");
                self.set_output(output);
                AppIntent::None
            }
            CommandAction::External(line) => {
                self.clear_output();
                self.set_status("working");
                AppIntent::Command(line)
            }
            CommandAction::Approve(id) => AppIntent::Approve(id),
            CommandAction::Deny(id) => AppIntent::Deny(id),
            CommandAction::Pause(id) => AppIntent::Pause(id),
            CommandAction::Resume(id) => AppIntent::Resume(id),
            CommandAction::Quit => AppIntent::Quit,
        }
    }

    fn approval_intent(
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

    fn session_intent(
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
}

fn latest_approval_id(state: &WorkspaceState) -> Option<String> {
    state.approvals.first().map(|approval| approval.id.clone())
}

fn active_session_id(state: &WorkspaceState) -> Option<String> {
    let id = state.session.id.trim();
    (!id.is_empty() && id != "workspace").then(|| id.to_string())
}
