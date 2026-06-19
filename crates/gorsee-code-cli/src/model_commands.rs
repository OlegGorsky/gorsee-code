use std::path::Path;

use anyhow::{anyhow, Result};
use gorsee_code_config::AgentConfig;
use gorsee_code_core::ModelCapability;

use crate::{
    args::{ModelsArgs, ModelsCommand, ModelsRecommendArgs},
    commands_extra::load_or_default,
    live,
};

pub fn run(root: &Path, env_key: Option<&str>, args: ModelsArgs) -> Result<String> {
    match args.command {
        None => list(root, env_key),
        Some(ModelsCommand::Benchmark) => benchmark(root, env_key),
        Some(ModelsCommand::Recommend(args)) => recommend(root, args),
    }
}

fn list(root: &Path, env_key: Option<&str>) -> Result<String> {
    if let Some(client) = live::client(root, env_key)? {
        return live::block_on(async move {
            let models = client.list_models().await?;
            Ok(render_live_models("models: live\n", &models))
        });
    }
    Ok(render_configured_models(
        root,
        "models: configured matrix\n",
    ))
}

fn benchmark(root: &Path, env_key: Option<&str>) -> Result<String> {
    if let Some(client) = live::client(root, env_key)? {
        return live::block_on(async move {
            let models = client.list_models().await?;
            Ok(render_live_models("models benchmark: live\n", &models))
        });
    }
    Ok(render_configured_models(
        root,
        "models benchmark: configured\n",
    ))
}

fn recommend(root: &Path, args: ModelsRecommendArgs) -> Result<String> {
    let config = load_or_default(root);
    let task = args.task.join(" ");
    let task = task.trim();
    let (agent_id, reason) = select_agent(task);
    let profile = config
        .agents
        .get(agent_id)
        .ok_or_else(|| anyhow!("agent profile missing: {agent_id}"))?;
    Ok(render_recommendation(task, agent_id, reason, profile))
}

fn render_live_models(header: &str, models: &[ModelCapability]) -> String {
    let mut out = header.to_string();
    for model in models {
        out.push_str(&format!(
            "- {} credit_multiplier={:.2} cost={} streaming={} tools={}\n",
            model.id,
            model.credit_multiplier,
            model.relative_cost_label(),
            model.supports_streaming,
            model.supports_tools
        ));
    }
    out
}

fn render_configured_models(root: &Path, header: &str) -> String {
    let config = load_or_default(root);
    let mut out = header.to_string();
    for (agent, profile) in config.agents {
        let multiplier = configured_multiplier(&profile.model);
        out.push_str(&format!(
            "- {agent} {} credit_multiplier={multiplier:.2} cost={}\n",
            profile.model,
            cost_label(multiplier)
        ));
    }
    out
}

fn render_recommendation(
    task: &str,
    agent_id: &str,
    reason: &str,
    profile: &AgentConfig,
) -> String {
    format!(
        "models recommend:\ntask={task}\nagent={agent_id}\nmodel={}\nreasoning={}\nreason={reason}\n",
        profile.model, profile.reasoning
    )
}

fn select_agent(task: &str) -> (&'static str, &'static str) {
    let task = task.to_lowercase();
    if contains_any(
        &task,
        &["bug", "fix", "code", "frontend", "backend", "patch"],
    ) {
        return ("coder", "code-change");
    }
    if contains_any(&task, &["test", "verify", "review", "lint"]) {
        return ("validator", "verification");
    }
    if contains_any(&task, &["find", "search", "read", "map"]) {
        return ("scout", "repository-research");
    }
    if contains_any(&task, &["summary", "summarize", "compress"]) {
        return ("summarizer", "context-compaction");
    }
    ("architect", "planning")
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn configured_multiplier(model: &str) -> f64 {
    match model {
        "neurogate/gpt-5" => 3.0,
        "neurogate/qwen-coder-fast" => 0.7,
        "neurogate/deepseek-coder" => 1.2,
        "neurogate/gpt-5-mini" => 1.0,
        "neurogate/cheap" => 0.25,
        _ => 1.0,
    }
}

fn cost_label(multiplier: f64) -> &'static str {
    match multiplier {
        n if n <= 0.75 => "cheap",
        n if n <= 1.5 => "standard",
        n if n <= 3.0 => "expensive",
        _ => "premium",
    }
}
