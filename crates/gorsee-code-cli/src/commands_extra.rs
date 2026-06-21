use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use gorsee_code_agent::{TaskRunSummary, TaskRunner};
use gorsee_code_config::{default_config, GorseeConfig};
use gorsee_code_core::{
    default_agent_matrix, preferred_model_ids, AgentProfile, AgentRole, ModelCapability, TaskSpec,
};
use gorsee_code_gateway::GatewayState;
use gorsee_code_hooks::builtin_hooks;
use gorsee_code_neurogate::{ChatMessage, ChatRequest, NeuroGateClient, NeuroGateError};
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

pub fn files(root: &Path) -> Result<String> {
    let registry = builtin_registry(root).context("build tool registry")?;
    let output = registry
        .run("list_files", serde_json::json!({ "max": 200 }))
        .context("list workspace files")?;
    if output.text.trim().is_empty() {
        return Ok("files: none\n".into());
    }
    Ok(format!("files:\n{}\n", output.text.trim_end()))
}

pub fn diff(root: &Path) -> Result<String> {
    let registry = builtin_registry(root).context("build tool registry")?;
    let output = registry
        .run("git_diff", serde_json::json!({}))
        .context("read git diff")?;
    if output.text.trim().is_empty() {
        return Ok("diff: clean\n".into());
    }
    Ok(format!("diff:\n{}\n", output.text.trim_end()))
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

pub fn execute(root: &Path, args: ObjectiveArgs, env_key: Option<&str>) -> Result<String> {
    let objective = args.objective.join(" ");
    let summary = run_task(root, objective, env_key)?;
    Ok(format!(
        "run: completed session={}\nevents={}\nagents={}\nartifacts={}\n",
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    ))
}

pub fn run_task(
    root: &Path,
    objective: impl Into<String>,
    env_key: Option<&str>,
) -> Result<TaskRunSummary> {
    let client = require_live_client(root, env_key)?;
    let agents = live_agent_matrix(&client)?;
    paths::ensure_layout(root)?;
    let spec = TaskSpec::new(objective, root.display().to_string());
    Ok(TaskRunner::new(paths::local_dir(root))
        .run_sequential_with_agents(&spec, &client, agents)?)
}

pub fn run_interactive_task(
    root: &Path,
    objective: impl Into<String>,
    env_key: Option<&str>,
) -> Result<TaskRunSummary> {
    let objective = objective.into();
    let client = require_live_client(root, env_key)?;
    let agents = live_interactive_agents(&client, &objective)?;
    paths::ensure_layout(root)?;
    let spec = TaskSpec::new(objective, root.display().to_string());
    Ok(TaskRunner::new(paths::local_dir(root))
        .run_sequential_with_agents(&spec, &client, agents)?)
}

pub fn run_skill(
    root: &Path,
    id: &str,
    objective: Vec<String>,
    env_key: Option<&str>,
) -> Result<String> {
    let client = require_live_client(root, env_key)?;
    let agents = live_agent_matrix(&client)?;
    let skill = find_skill(id).ok_or_else(|| anyhow!("unknown skill: {id}"))?;
    paths::ensure_layout(root)?;
    let objective = if objective.is_empty() {
        skill.instructions.clone()
    } else {
        objective.join(" ")
    };
    let spec = TaskSpec::new(objective, root.display().to_string());
    let summary = TaskRunner::new(paths::local_dir(root))
        .run_skill_with_agents(&spec, &skill.id, &client, agents)?;
    Ok(format!(
        "skill: {} session={}\nevents={}\nagents={}\nartifacts={}\n",
        skill.id,
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    ))
}

pub(crate) fn require_live_client(root: &Path, env_key: Option<&str>) -> Result<NeuroGateClient> {
    live::client(root, env_key)?.ok_or_else(missing_auth)
}

fn live_agent_matrix(client: &NeuroGateClient) -> Result<Vec<AgentProfile>> {
    let models = live::block_on(async { Ok(client.list_models().await?) })?;
    let mut health = BTreeMap::new();
    let mut agents = default_agent_matrix();
    for agent in &mut agents {
        agent.model = select_live_model(client, &agent.role, &models, &mut health)?;
    }
    Ok(agents)
}

fn live_interactive_agents(client: &NeuroGateClient, objective: &str) -> Result<Vec<AgentProfile>> {
    if is_simple_chat(objective) {
        return live_primary_agent(client);
    }
    live_agent_matrix(client)
}

fn live_primary_agent(client: &NeuroGateClient) -> Result<Vec<AgentProfile>> {
    let models = live::block_on(async { Ok(client.list_models().await?) })?;
    let mut health = BTreeMap::new();
    let mut agent = default_agent_matrix()
        .into_iter()
        .find(|profile| profile.role == AgentRole::Architect)
        .ok_or_else(|| anyhow!("architect agent profile is missing"))?;
    agent.model = select_live_model(client, &agent.role, &models, &mut health)?;
    agent.reasoning = "low".into();
    agent.tools.clear();
    agent.budget_tokens = agent.budget_tokens.min(8_000);
    Ok(vec![agent])
}

fn is_simple_chat(objective: &str) -> bool {
    let value = objective.trim().to_lowercase();
    if value.is_empty() || value.len() > 180 || value.lines().count() > 2 {
        return false;
    }
    if value.starts_with('/') {
        return false;
    }
    if task_like_words().iter().any(|word| value.contains(word)) {
        return false;
    }
    simple_chat_prefixes()
        .iter()
        .any(|prefix| value.starts_with(prefix))
        || value.split_whitespace().count() <= 18
}

fn simple_chat_prefixes() -> &'static [&'static str] {
    &[
        "привет",
        "здравств",
        "добрый день",
        "доброе утро",
        "добрый вечер",
        "hello",
        "hi",
        "hey",
        "как дела",
        "спасибо",
        "ок",
        "ладно",
        "понял",
    ]
}

fn task_like_words() -> &'static [&'static str] {
    &[
        "добав",
        "исправ",
        "почин",
        "сдел",
        "созда",
        "реализ",
        "проверь",
        "запусти",
        "открой",
        "найди",
        "удали",
        "перенеси",
        "закомить",
        "зарелиз",
        "файл",
        "папк",
        "код",
        "тест",
        "ошиб",
        "bug",
        "fix",
        "commit",
        "release",
        "run",
    ]
}

fn select_live_model(
    client: &NeuroGateClient,
    role: &AgentRole,
    models: &[ModelCapability],
    health: &mut BTreeMap<String, bool>,
) -> Result<String> {
    let mut rejected = Vec::new();
    for model in candidate_model_ids(role, models) {
        if model_accepts_chat(client, &model, health)? {
            return Ok(model);
        }
        rejected.push(model);
    }
    Err(anyhow!(
        "no usable live NeuroGate model for {}; rejected={}",
        role.id(),
        rejected.join(",")
    ))
}

fn candidate_model_ids(role: &AgentRole, models: &[ModelCapability]) -> Vec<String> {
    let available = models
        .iter()
        .map(|model| model.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for model in preferred_model_ids(role) {
        if available.contains(model) {
            candidates.push((*model).to_string());
        }
    }
    for model in models {
        if !candidates.iter().any(|candidate| candidate == &model.id) {
            candidates.push(model.id.clone());
        }
    }
    candidates
}

fn model_accepts_chat(
    client: &NeuroGateClient,
    model: &str,
    health: &mut BTreeMap<String, bool>,
) -> Result<bool> {
    if let Some(usable) = health.get(model) {
        return Ok(*usable);
    }
    let request = ChatRequest::new(
        model,
        vec![
            ChatMessage {
                role: "system".into(),
                content: "Reply with OK only.".into(),
            },
            ChatMessage::user("OK"),
        ],
    );
    let usable = match live::block_on(async {
        Ok::<_, anyhow::Error>(client.chat_completion(&request).await)
    })? {
        Ok(_) => true,
        Err(error) if is_model_rejection(&error) => false,
        Err(error) => {
            return Err(anyhow!(error)).with_context(|| format!("probe live model {model}"));
        }
    };
    health.insert(model.to_string(), usable);
    Ok(usable)
}

fn is_model_rejection(error: &NeuroGateError) -> bool {
    matches!(
        error,
        NeuroGateError::Status { status: 400, body, .. } if body.contains("upstream_rejected")
    )
}

fn missing_auth() -> anyhow::Error {
    anyhow!("missing_auth: run `gcode` and enter a NeuroGate API key or set NEUROGATE_API_KEY")
}

pub fn config_check(root: &Path) -> String {
    match GorseeConfig::load(paths::config_path(root)) {
        Ok(_) => "config: ok\n".into(),
        Err(gorsee_code_config::config::ConfigError::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            "config: default\n".into()
        }
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
    latest_session_id(root)?.ok_or_else(|| anyhow!("no sessions found"))
}

pub fn latest_session_id(root: &Path) -> Result<Option<String>> {
    Ok(session_ids_by_started_at(root)?.pop())
}

pub fn session_ids_by_started_at(root: &Path) -> Result<Vec<String>> {
    let mut sessions = Vec::new();
    for id in session_ids(root)? {
        let manifest =
            read_manifest(root, &id).with_context(|| format!("read session {id} manifest"))?;
        sessions.push((manifest.started_at, id));
    }
    sessions.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    Ok(sessions.into_iter().map(|(_, id)| id).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_models_prefer_role_order_before_other_live_models() {
        let models = vec![
            model("qwen3.7-max"),
            model("deepseek-v4-pro"),
            model("custom-model"),
            model("glm-5.1"),
        ];

        assert_eq!(
            candidate_model_ids(&AgentRole::Architect, &models),
            ["glm-5.1", "deepseek-v4-pro", "qwen3.7-max", "custom-model"]
        );
    }

    #[test]
    fn upstream_rejected_status_is_model_rejection() {
        let rejected = NeuroGateError::Status {
            status: 400,
            url: "https://api.neurogate.space/v1/chat/completions".into(),
            body: r#"{"error":{"code":"upstream_rejected"}}"#.into(),
        };
        let auth = NeuroGateError::Status {
            status: 401,
            url: "https://api.neurogate.space/v1/chat/completions".into(),
            body: r#"{"error":{"code":"unauthorized"}}"#.into(),
        };

        assert!(is_model_rejection(&rejected));
        assert!(!is_model_rejection(&auth));
    }

    #[test]
    fn short_greetings_use_interactive_chat_route() {
        assert!(is_simple_chat("Привет"));
        assert!(is_simple_chat("hello, как дела?"));
        assert!(is_simple_chat("почему так долго отвечаешь?"));
        assert!(!is_simple_chat("Привет, исправь тесты"));
        assert!(!is_simple_chat("Сделай аудит проекта"));
        assert!(!is_simple_chat("/project /home/oleg"));
    }

    fn model(id: &str) -> ModelCapability {
        ModelCapability {
            id: id.into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 1.0,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        }
    }
}
