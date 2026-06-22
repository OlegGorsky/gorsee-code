use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use gorsee_code_coding_core::{LocalCodingProtocol, TurnRequest, WorkspaceRef};
use gorsee_code_core::{default_agent_matrix, AgentProfile};
use gorsee_code_neurogate::NeuroGateClient;

use crate::{commands_extra::select_live_model, live};

pub(crate) fn live_turn_agents(
    client: &NeuroGateClient,
    root: &Path,
    objective: &str,
) -> Result<Vec<AgentProfile>> {
    live_agents_for_profiles(client, planned_profiles(root, objective))
}

fn live_agents_for_profiles(
    client: &NeuroGateClient,
    profiles: Vec<AgentProfile>,
) -> Result<Vec<AgentProfile>> {
    let models = live::block_on(async { Ok(client.list_models().await?) })?;
    let mut health = BTreeMap::new();
    let mut agents = Vec::new();
    for mut agent in profiles {
        agent.model = select_live_model(client, &agent.role, &models, &mut health)?;
        agents.push(agent);
    }
    Ok(agents)
}

pub(crate) fn planned_profiles(root: &Path, objective: &str) -> Vec<AgentProfile> {
    LocalCodingProtocol::default()
        .plan_turn(turn_request(root, objective), default_agent_matrix())
        .orchestration
        .agents
}

fn turn_request(root: &Path, objective: &str) -> TurnRequest {
    TurnRequest {
        workspace: WorkspaceRef {
            root: root.display().to_string(),
            branch: None,
            session_id: None,
        },
        message: objective.into(),
        user_id: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gorsee_code_core::AgentRole;

    #[test]
    fn interactive_chat_route_uses_single_primary_agent() {
        let agents = super::planned_profiles(Path::new("/tmp/project"), "Привет");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].role, AgentRole::Architect);
        assert!(agents[0].tools.is_empty());
        assert!(agents[0].budget_tokens <= 8_000);
    }

    #[test]
    fn interactive_edit_route_is_not_full_matrix() {
        let agents = super::planned_profiles(Path::new("/tmp/project"), "напиши модуль diff");
        let roles = agents
            .into_iter()
            .map(|agent| agent.role)
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            vec![AgentRole::Architect, AgentRole::Coder, AgentRole::Validator]
        );
    }

    #[test]
    fn interactive_review_route_uses_review_agents_only() {
        let agents = super::planned_profiles(Path::new("/tmp/project"), "покажи diff");
        let roles = agents
            .into_iter()
            .map(|agent| agent.role)
            .collect::<Vec<_>>();
        assert_eq!(roles, vec![AgentRole::Validator, AgentRole::Summarizer]);
    }
}
