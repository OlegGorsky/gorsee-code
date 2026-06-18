use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Architect,
    Scout,
    Coder,
    Validator,
    Summarizer,
}

impl AgentRole {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Architect => "architect",
            Self::Scout => "scout",
            Self::Coder => "coder",
            Self::Validator => "validator",
            Self::Summarizer => "summarizer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Planning,
    Reading,
    Patching,
    Validating,
    WaitingApproval,
    Finished,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub role: AgentRole,
    pub model: String,
    pub reasoning: String,
    pub tools: Vec<String>,
    pub budget_tokens: u64,
    pub temperature: f32,
}

impl AgentProfile {
    pub fn id(&self) -> &'static str {
        self.role.id()
    }
}

pub fn default_agent_matrix() -> Vec<AgentProfile> {
    vec![
        architect_profile(),
        scout_profile(),
        coder_profile(),
        validator_profile(),
        summarizer_profile(),
    ]
}

fn architect_profile() -> AgentProfile {
    profile(
        AgentRole::Architect,
        "neurogate/gpt-5",
        "high",
        &["read", "search", "repo_map"],
        40_000,
        0.2,
    )
}

fn scout_profile() -> AgentProfile {
    profile(
        AgentRole::Scout,
        "neurogate/qwen-coder-fast",
        "low",
        &["read", "search", "repo_map"],
        12_000,
        0.1,
    )
}

fn coder_profile() -> AgentProfile {
    profile(
        AgentRole::Coder,
        "neurogate/deepseek-coder",
        "medium",
        &["read", "search", "propose_patch", "run_test"],
        50_000,
        0.15,
    )
}

fn validator_profile() -> AgentProfile {
    profile(
        AgentRole::Validator,
        "neurogate/gpt-5-mini",
        "medium",
        &["read", "diff", "run_test"],
        20_000,
        0.1,
    )
}

fn summarizer_profile() -> AgentProfile {
    profile(
        AgentRole::Summarizer,
        "neurogate/cheap",
        "off",
        &["read_events", "write_summary"],
        8_000,
        0.0,
    )
}

fn profile(
    role: AgentRole,
    model: &str,
    reasoning: &str,
    tools: &[&str],
    budget_tokens: u64,
    temperature: f32,
) -> AgentProfile {
    AgentProfile {
        role,
        model: model.into(),
        reasoning: reasoning.into(),
        tools: tools.iter().map(|tool| (*tool).into()).collect(),
        budget_tokens,
        temperature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matrix_contains_expected_roles() {
        let roles: Vec<_> = default_agent_matrix()
            .into_iter()
            .map(|agent| agent.id())
            .collect();
        assert_eq!(
            roles,
            ["architect", "scout", "coder", "validator", "summarizer"]
        );
    }
}
