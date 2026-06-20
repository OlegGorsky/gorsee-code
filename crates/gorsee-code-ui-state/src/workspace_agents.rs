use std::{collections::BTreeMap, fs, path::Path};

use gorsee_code_config::{default_config, GorseeConfig};
use gorsee_code_core::{default_agent_matrix, AgentStatus};
use gorsee_code_usage::TokenLedger;

use crate::{AgentView, BudgetView};

pub(super) fn agent_views(
    root: &Path,
    status: &str,
    _used_tokens: u64,
    ledger: Option<&TokenLedger>,
) -> Vec<AgentView> {
    let config = config_for(root);
    let by_agent = ledger.map(TokenLedger::by_agent).unwrap_or_default();
    let default_ids = default_agent_matrix()
        .into_iter()
        .map(|profile| profile.id().to_string())
        .collect::<Vec<_>>();
    let ordered = ordered_agent_ids(&default_ids, &config.agents);
    ordered
        .iter()
        .enumerate()
        .filter_map(|(index, id)| {
            let profile = config.agents.get(id)?;
            let tokens = by_agent.get(id).map(|totals| totals.tokens).unwrap_or(0);
            Some(AgentView::from_parts(
                id,
                &profile.model,
                agent_status(status, index),
                tokens,
                profile.budget_tokens,
            ))
        })
        .collect()
}

pub(super) fn budget_view(used_tokens: u64, limit_tokens: u64) -> BudgetView {
    let percent_used = if limit_tokens == 0 {
        100.0
    } else {
        used_tokens as f64 * 100.0 / limit_tokens as f64
    };
    BudgetView {
        used_tokens,
        limit_tokens,
        percent_used,
        warning: percent_used >= 75.0,
        stopped: percent_used >= 100.0,
    }
}

pub(super) fn config_for(root: &Path) -> GorseeConfig {
    GorseeConfig::load(root.join("gorsee-code.toml"))
        .unwrap_or_else(|_| default_config(project_name(root)))
}

pub(super) fn read_ledger(root: &Path, session_id: &str) -> Option<TokenLedger> {
    let text = fs::read_to_string(
        root.join(".gorsee-code")
            .join("sessions")
            .join(session_id)
            .join("token-ledger.json"),
    )
    .ok()?;
    serde_json::from_str(&text).ok()
}

fn ordered_agent_ids(
    default_ids: &[String],
    agents: &BTreeMap<String, gorsee_code_config::AgentConfig>,
) -> Vec<String> {
    let mut ids = default_ids
        .iter()
        .filter(|id| agents.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let extra = agents
        .keys()
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    ids.extend(extra);
    ids
}

fn project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("gorsee-code")
        .to_string()
}

fn agent_status(status: &str, index: usize) -> AgentStatus {
    match status {
        "finished" => AgentStatus::Finished,
        "failed" => AgentStatus::Failed,
        "waiting_approval" => AgentStatus::WaitingApproval,
        "running" => running_status(index),
        _ => AgentStatus::Idle,
    }
}

fn running_status(index: usize) -> AgentStatus {
    match index {
        0 => AgentStatus::Planning,
        _ => AgentStatus::Idle,
    }
}
