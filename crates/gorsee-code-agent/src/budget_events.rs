use gorsee_code_core::{AgentProfile, EventKind};
use gorsee_code_hooks::{HookBus, HookContext, HookPoint};
use gorsee_code_neurogate::ChatResponse;
use gorsee_code_session::SessionManifest;
use gorsee_code_usage::{BudgetPolicy, TokenLedger, UsageRecord};
use serde_json::{json, Value};

use crate::{events::EventSink, AgentRunError};

pub(crate) fn usage_record_from_response(
    agent: &AgentProfile,
    response: &ChatResponse,
) -> Option<UsageRecord> {
    let usage = response.usage.as_ref()?;
    let cached_tokens = nested_u64(usage, &["prompt_tokens_details", "cached_tokens"])
        .or_else(|| nested_u64(usage, &["input_tokens_details", "cached_tokens"]))
        .unwrap_or(0);
    let reasoning_tokens = nested_u64(usage, &["completion_tokens_details", "reasoning_tokens"])
        .or_else(|| nested_u64(usage, &["output_tokens_details", "reasoning_tokens"]))
        .unwrap_or(0);
    let input_tokens = first_u64(usage, &["prompt_tokens", "input_tokens"]);
    let output_tokens = first_u64(usage, &["completion_tokens", "output_tokens"]);
    let total_tokens = usage.get("total_tokens").and_then(Value::as_u64);
    let main_tokens = main_tokens(
        input_tokens,
        output_tokens,
        total_tokens,
        UsageDetails {
            cached_tokens,
            reasoning_tokens,
        },
    )?;

    Some(UsageRecord {
        agent_id: agent.id().to_string(),
        phase: "agent".into(),
        model: agent.model.clone(),
        input_tokens: main_tokens.input_tokens,
        output_tokens: main_tokens.output_tokens,
        cached_tokens,
        reasoning_tokens,
        estimated: false,
        credit_multiplier: 1.0,
    })
}

pub(crate) fn sync_manifest_budget(manifest: &mut SessionManifest, usage_records: &[UsageRecord]) {
    manifest.budget.tokens_used = ledger_from(usage_records).totals().tokens;
}

pub(crate) fn record_budget_status(
    sink: &mut EventSink<'_>,
    manifest: &SessionManifest,
    usage_records: &[UsageRecord],
) -> Result<(), AgentRunError> {
    let status = budget_policy(manifest).evaluate(&ledger_from(usage_records));
    if !status.warning && !status.stopped {
        return Ok(());
    }
    let hook = HookBus::default().run(
        HookPoint::OnBudgetWarning,
        HookContext {
            budget: Some(status.clone()),
            ..HookContext::default()
        },
    );
    sink.push(
        None,
        budget_event_kind(status.stopped, hook.blocked),
        json!({
            "used_tokens": status.used_tokens,
            "limit_tokens": status.limit_tokens,
            "percent_used": status.percent_used,
            "hook_messages": hook.messages
        }),
    )?;
    if status.stopped || hook.blocked {
        return Err(AgentRunError::Runtime("budget exceeded".into()));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct UsageDetails {
    cached_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy)]
struct MainTokens {
    input_tokens: u64,
    output_tokens: u64,
}

fn main_tokens(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    details: UsageDetails,
) -> Option<MainTokens> {
    if input_tokens.is_some() || output_tokens.is_some() {
        return Some(MainTokens {
            input_tokens: input_tokens
                .unwrap_or(0)
                .saturating_sub(details.cached_tokens),
            output_tokens: output_tokens
                .unwrap_or(0)
                .saturating_sub(details.reasoning_tokens),
        });
    }
    Some(MainTokens {
        input_tokens: total_tokens?
            .saturating_sub(details.cached_tokens)
            .saturating_sub(details.reasoning_tokens),
        output_tokens: 0,
    })
}

fn budget_policy(manifest: &SessionManifest) -> BudgetPolicy {
    BudgetPolicy {
        session_tokens: manifest.budget.tokens_limit,
        session_usd: None,
        warn_at_percent: 75,
        stop_at_percent: 100,
    }
}

fn budget_event_kind(stopped: bool, blocked: bool) -> EventKind {
    if stopped || blocked {
        EventKind::BudgetExceeded
    } else {
        EventKind::BudgetWarning
    }
}

fn ledger_from(usage_records: &[UsageRecord]) -> TokenLedger {
    TokenLedger {
        records: usage_records.to_vec(),
    }
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn nested_u64(value: &Value, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_u64)
}
