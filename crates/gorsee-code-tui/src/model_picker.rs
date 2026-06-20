use gorsee_code_core::{preferred_model_ids, AgentRole};
use gorsee_code_ui_state::WorkspaceState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelChoice {
    agent: String,
    options: Vec<String>,
    selected: usize,
}

impl ModelChoice {
    pub(crate) fn agent(&self) -> &str {
        &self.agent
    }

    pub(crate) fn model(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub(crate) fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub(crate) fn select_next(&mut self) {
        if self.selected + 1 < self.options.len() {
            self.selected += 1;
        }
    }
}

pub(crate) fn choices_from_state(state: &WorkspaceState) -> Vec<ModelChoice> {
    state
        .agents
        .iter()
        .map(|agent| choice_for_agent(&agent.id, &agent.model))
        .collect()
}

fn choice_for_agent(agent: &str, current: &str) -> ModelChoice {
    let mut options = role_for_agent(agent)
        .map(|role| {
            preferred_model_ids(&role)
                .iter()
                .map(|model| (*model).to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![current.to_string()]);
    if !options.iter().any(|model| model == current) {
        options.insert(0, current.to_string());
    }
    let selected = options
        .iter()
        .position(|model| model == current)
        .unwrap_or(0);
    ModelChoice {
        agent: agent.to_string(),
        options,
        selected,
    }
}

fn role_for_agent(agent: &str) -> Option<AgentRole> {
    match agent {
        "architect" => Some(AgentRole::Architect),
        "scout" => Some(AgentRole::Scout),
        "coder" => Some(AgentRole::Coder),
        "validator" => Some(AgentRole::Validator),
        "summarizer" => Some(AgentRole::Summarizer),
        _ => None,
    }
}
