use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use crate::{args::BudgetSetArgs, config_file};

pub fn set(root: &Path, args: BudgetSetArgs) -> Result<String> {
    let mut config = config_file::load_editable(root)?;
    let mut out = "budget: updated\n".to_string();

    if let Some(value) = args.session {
        let tokens = parse_tokens(&value)?;
        config.budget.session_tokens = tokens;
        out.push_str(&format!("session_tokens={tokens}\n"));
    }

    if let Some(agent) = args.agent {
        let (agent_id, tokens) = parse_agent_limit(&agent)?;
        config
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow!("unknown agent: {agent_id}"))?
            .budget_tokens = tokens;
        out.push_str(&format!("agent {agent_id}={tokens}\n"));
    }

    if out.lines().count() == 1 {
        bail!("budget set needs --session or --agent");
    }

    config_file::save(root, &config)?;
    Ok(out)
}

fn parse_agent_limit(values: &[String]) -> Result<(&str, u64)> {
    let [agent, limit] = values else {
        bail!("--agent needs an agent id and token limit");
    };
    Ok((agent.as_str(), parse_tokens(limit)?))
}

fn parse_tokens(value: &str) -> Result<u64> {
    let trimmed = value.trim().replace('_', "");
    let (number, multiplier) = match trimmed.as_bytes().last().copied() {
        Some(b'k' | b'K') => (&trimmed[..trimmed.len() - 1], 1_000),
        Some(b'm' | b'M') => (&trimmed[..trimmed.len() - 1], 1_000_000),
        Some(_) => (trimmed.as_str(), 1),
        None => bail!("token limit is empty"),
    };
    if number.is_empty() {
        bail!("token limit is empty");
    }
    number
        .parse::<u64>()
        .context("parse token limit")?
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow!("token limit is too large"))
}
