use std::collections::BTreeMap;

use gorsee_code_core::default_agent_matrix;

use crate::{AgentConfig, AuthSource, BudgetConfig, GorseeConfig, NeuroGateConfig, ProjectConfig};

pub fn default_config(project_name: impl Into<String>) -> GorseeConfig {
    let mut agents = BTreeMap::new();
    for profile in default_agent_matrix() {
        agents.insert(
            profile.id().to_string(),
            AgentConfig {
                model: profile.model,
                reasoning: profile.reasoning,
                tools: profile.tools,
                budget_tokens: profile.budget_tokens,
                temperature: profile.temperature,
            },
        );
    }

    GorseeConfig {
        project: ProjectConfig {
            name: project_name.into(),
            guidance_files: vec!["AGENTS.md".into(), "GORSEE.md".into(), "README.md".into()],
            protected_paths: Vec::new(),
        },
        neurogate: NeuroGateConfig {
            endpoint: "https://api.neurogate.example/v1".into(),
            auth_source: AuthSource::Env,
        },
        budget: BudgetConfig {
            session_tokens: 80_000,
            session_usd: 2.0,
            warn_at_percent: 75,
            stop_at_percent: 100,
        },
        agents,
    }
}

pub fn default_config_toml(project_name: impl Into<String>) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(&default_config(project_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_contains_agent_matrix() {
        let config = default_config("workspace");
        assert!(config.agents.contains_key("architect"));
        assert!(config.agents.contains_key("coder"));
    }
}
