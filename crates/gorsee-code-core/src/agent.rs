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

pub fn preferred_model_ids(role: &AgentRole) -> &'static [&'static str] {
    match role {
        AgentRole::Architect => &[
            "glm-5.1",
            "kimi-k2.6",
            "deepseek-v4-pro",
            "gpt-5.4",
            "gpt-5.5",
            "qwen3.7-max",
        ],
        AgentRole::Scout => &[
            "vibe-lite-1",
            "mimo-v2.5",
            "deepseek-v4-flash",
            "qwen3.7-plus",
        ],
        AgentRole::Coder => &[
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "kimi-k2.6",
            "qwen3.7-plus",
        ],
        AgentRole::Validator => &[
            "kimi-k2.6",
            "glm-5.1",
            "deepseek-v4-pro",
            "gpt-5.4-mini",
            "qwen3.7-plus",
        ],
        AgentRole::Summarizer => &["vibe-lite-1", "mimo-v2.5", "deepseek-v4-flash"],
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
        preferred_model_ids(&AgentRole::Architect)[0],
        "high",
        &["read", "search", "repo_map", "mcp"],
        40_000,
        0.2,
    )
}

fn scout_profile() -> AgentProfile {
    profile(
        AgentRole::Scout,
        preferred_model_ids(&AgentRole::Scout)[0],
        "low",
        &["read", "search", "repo_map", "mcp"],
        12_000,
        0.1,
    )
}

fn coder_profile() -> AgentProfile {
    profile(
        AgentRole::Coder,
        preferred_model_ids(&AgentRole::Coder)[0],
        "medium",
        &["read", "search", "propose_patch", "run_test", "mcp"],
        50_000,
        0.15,
    )
}

fn validator_profile() -> AgentProfile {
    profile(
        AgentRole::Validator,
        preferred_model_ids(&AgentRole::Validator)[0],
        "medium",
        &["read", "diff", "run_test"],
        20_000,
        0.1,
    )
}

fn summarizer_profile() -> AgentProfile {
    profile(
        AgentRole::Summarizer,
        preferred_model_ids(&AgentRole::Summarizer)[0],
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
