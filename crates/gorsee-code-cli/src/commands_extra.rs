use std::{fs, path::Path};

use anyhow::{anyhow, Context, Result};
use gorsee_code_agent::{MissionRunSummary, MissionRunner};
use gorsee_code_config::{default_config, GorseeConfig};
use gorsee_code_core::{default_agent_matrix, MissionSpec};
use gorsee_code_gateway::GatewayState;
use gorsee_code_hooks::builtin_hooks;
use gorsee_code_neurogate::NeuroGateClient;
use gorsee_code_session::SessionManifest;
use gorsee_code_skills::find_skill;
use gorsee_code_tools::builtin_registry;
use gorsee_code_ui_state::workspace_state;

use crate::{args::ObjectiveArgs, live, paths};

pub fn agents() -> Result<String> {
    let mut out = "agents:\n".to_string();
    for agent in default_agent_matrix() {
        out.push_str(&format!(
            "- {} model={} reasoning={} budget_tokens={}\n",
            agent.id(),
            agent.model,
            agent.reasoning,
            agent.budget_tokens
        ));
    }
    Ok(out)
}

pub fn usage(root: &Path) -> Result<String> {
    let state = workspace_state(root);
    Ok(format!(
        "usage: current tokens={}/{} percent={:.1}\n",
        state.budget.used_tokens, state.budget.limit_tokens, state.budget.percent_used
    ))
}

pub fn tools(root: &Path) -> Result<String> {
    let registry = builtin_registry(root).context("build tool registry")?;
    let mut out = "tools:\n".to_string();
    for tool in registry.manifests() {
        out.push_str(&format!("- {} risk={:?}\n", tool.name, tool.risk));
    }
    Ok(out)
}

pub fn hooks() -> Result<String> {
    let mut out = "hooks:\n".to_string();
    for hook in builtin_hooks() {
        out.push_str(&format!("- {} {:?}\n", hook.id, hook.point));
    }
    Ok(out)
}

pub fn capabilities(root: &Path, env_key: Option<&str>) -> Result<String> {
    if let Some(client) = live::client(root, env_key)? {
        return live::block_on(async move {
            let models = client.list_models().await?;
            Ok(format!("capabilities: live models={}\n", models.len()))
        });
    }
    let state = GatewayState::workspace(root);
    Ok(format!(
        "capabilities: configured models={}\n",
        state.capabilities.len()
    ))
}

pub fn mission(root: &Path, args: ObjectiveArgs, env_key: Option<&str>) -> Result<String> {
    let objective = args.objective.join(" ");
    let summary = run_mission(root, objective, env_key)?;
    Ok(format!(
        "mission: completed session={}\nevents={}\nagents={}\nartifacts={}\n",
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    ))
}

pub fn run_mission(
    root: &Path,
    objective: impl Into<String>,
    env_key: Option<&str>,
) -> Result<MissionRunSummary> {
    let client = require_live_client(root, env_key)?;
    paths::ensure_layout(root)?;
    let spec = MissionSpec::new(objective, root.display().to_string());
    Ok(MissionRunner::new(paths::local_dir(root)).run_sequential(&spec, &client)?)
}

pub fn skill_mission(
    root: &Path,
    id: &str,
    objective: Vec<String>,
    env_key: Option<&str>,
) -> Result<String> {
    let client = require_live_client(root, env_key)?;
    let skill = find_skill(id).ok_or_else(|| anyhow!("unknown skill: {id}"))?;
    paths::ensure_layout(root)?;
    let objective = if objective.is_empty() {
        skill.instructions.clone()
    } else {
        objective.join(" ")
    };
    let spec = MissionSpec::new(objective, root.display().to_string());
    let summary =
        MissionRunner::new(paths::local_dir(root)).run_skill(&spec, &skill.id, &client)?;
    Ok(format!(
        "skill: {} session={}\nevents={}\nagents={}\nartifacts={}\n",
        skill.id,
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    ))
}

fn require_live_client(root: &Path, env_key: Option<&str>) -> Result<NeuroGateClient> {
    live::client(root, env_key)?.ok_or_else(missing_auth)
}

fn missing_auth() -> anyhow::Error {
    anyhow!("missing_auth: run `gcode` and enter a NeuroGate API key or set NEUROGATE_API_KEY")
}

pub fn config_check(root: &Path) -> String {
    match GorseeConfig::load(paths::config_path(root)) {
        Ok(_) => "config: ok\n".into(),
        Err(error) => format!("config: error {error}\n"),
    }
}

pub fn load_or_default(root: &Path) -> GorseeConfig {
    GorseeConfig::load(paths::config_path(root))
        .unwrap_or_else(|_| default_config(paths::project_name(root)))
}

pub fn resolve_session_id(root: &Path, requested: Option<String>) -> Result<String> {
    if let Some(id) = requested {
        return Ok(id);
    }
    session_ids(root)?
        .into_iter()
        .max()
        .ok_or_else(|| anyhow!("no sessions found"))
}

pub fn session_ids(root: &Path) -> Result<Vec<String>> {
    let dir = paths::sessions_dir(root);
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read sessions directory"),
    };
    Ok(entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>())
}

pub fn read_manifest(root: &Path, id: &str) -> Result<SessionManifest> {
    let path = paths::sessions_dir(root).join(id).join("manifest.json");
    let text = fs::read_to_string(path).context("read session manifest")?;
    serde_json::from_str(&text).context("parse session manifest")
}
