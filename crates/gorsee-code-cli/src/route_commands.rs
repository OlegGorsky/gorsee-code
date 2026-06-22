use std::path::Path;

use anyhow::Result;
use gorsee_code_coding_core::{CodingIntent, LocalCodingProtocol, TurnRequest, WorkspaceRef};
use gorsee_code_core::{default_agent_matrix, AgentProfile};

use crate::{args::ObjectiveArgs, config_file};

pub fn explain(root: &Path, args: ObjectiveArgs) -> Result<String> {
    let config = config_file::load_editable(root)?;
    let objective = args.objective.join(" ");
    let profiles = configured_profiles(&config);
    let snapshot =
        LocalCodingProtocol::default().plan_turn(turn_request(root, &objective), profiles);
    let mut out = format!("route: ready\nobjective: {objective}\n");
    out.push_str(&format!(
        "intent: {}\n",
        intent_label(snapshot.orchestration.intent.intent)
    ));

    for agent in snapshot.orchestration.agents {
        out.push_str(&format!(
            "- {} {} reasoning={} budget_tokens={}\n",
            agent.id(),
            agent.model,
            agent.reasoning,
            agent.budget_tokens
        ));
    }

    Ok(out)
}

fn intent_label(intent: CodingIntent) -> &'static str {
    match intent {
        CodingIntent::Chat => "chat",
        CodingIntent::Inspect => "inspect",
        CodingIntent::Edit => "edit",
        CodingIntent::Test => "test",
        CodingIntent::Review => "review",
        CodingIntent::Release => "release",
    }
}

fn configured_profiles(config: &gorsee_code_config::GorseeConfig) -> Vec<AgentProfile> {
    let mut profiles = default_agent_matrix();
    for profile in &mut profiles {
        if let Some(configured) = config.agents.get(profile.id()) {
            profile.model = configured.model.clone();
            profile.reasoning = configured.reasoning.clone();
            profile.tools = configured.tools.clone();
            profile.budget_tokens = configured.budget_tokens;
            profile.temperature = configured.temperature;
        }
    }
    profiles
}

fn turn_request(root: &Path, objective: &str) -> TurnRequest {
    TurnRequest {
        workspace: WorkspaceRef {
            root: root.display().to_string(),
            branch: None,
            session_id: None,
        },
        message: objective.into(),
        user_id: Some("cli".into()),
    }
}
