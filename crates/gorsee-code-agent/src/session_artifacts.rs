use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use gorsee_code_artifacts::{ArtifactRecord, ArtifactStore};
use gorsee_code_coding_core::{ExecutionContract, TurnPlan};
use gorsee_code_core::TaskSpec;
use gorsee_code_diff::workspace_diff;
use gorsee_code_safety::{OutputBounds, Redactor};
use gorsee_code_session::SessionManifest;
use gorsee_code_usage::{TokenLedger, TokenTotals, UsageRecord};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    artifact_signals::{
        diff_signal_not_required, diff_signal_ok, diff_signal_unavailable, verification_signal,
        DiffSignal, VerificationSignal,
    },
    protocol::{AgentAnswer, ToolResult},
    report::write_report,
    AgentRunError,
};

pub(crate) struct RunArtifacts {
    pub(crate) records: Vec<ArtifactRecord>,
    pub(crate) diff: DiffSignal,
    pub(crate) verification: VerificationSignal,
}

pub(crate) struct RunArtifactsInput<'a> {
    pub(crate) session_dir: &'a Path,
    pub(crate) manifest: &'a SessionManifest,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) answers: &'a [AgentAnswer],
    pub(crate) results: &'a [ToolResult],
    pub(crate) usage_records: &'a [UsageRecord],
    pub(crate) plan: Option<&'a TurnPlan>,
    pub(crate) contract: &'a ExecutionContract,
}

pub(crate) fn write_run_artifacts(
    input: RunArtifactsInput<'_>,
) -> Result<RunArtifacts, AgentRunError> {
    let RunArtifactsInput {
        session_dir,
        manifest,
        spec,
        skill_id,
        answers,
        results,
        usage_records,
        plan,
        contract,
    } = input;
    let store = ArtifactStore::new(session_dir.join("artifacts"));
    let mut artifacts = vec![write_report(session_dir, spec, skill_id, answers, results)?];
    artifacts.push(write_json(
        &store,
        "usage.json",
        &usage_snapshot(manifest, answers, results, usage_records),
    )?);
    artifacts.push(write_json(
        &store,
        "limits-start.json",
        &limits_snapshot("start", manifest, usage_records),
    )?);
    artifacts.push(write_json(
        &store,
        "limits-end.json",
        &limits_snapshot("end", manifest, usage_records),
    )?);
    let diff_required = contract.diff_required || results_require_diff(results);
    let (diff_artifact, diff) = write_structured_diff(&store, spec, diff_required)?;
    artifacts.push(diff_artifact);
    if diff_required {
        artifacts.push(write_diff(&store, spec)?);
    }
    let (verification_artifact, verification) =
        write_verification(&store, results, contract.verification_required)?;
    artifacts.push(verification_artifact);
    artifacts.push(write_json(&store, "execution-contract.json", &contract)?);
    if let Some(plan) = plan {
        artifacts.push(write_json(&store, "plan.json", &plan)?);
    }
    Ok(RunArtifacts {
        records: artifacts,
        diff,
        verification,
    })
}

pub(crate) fn write_session_snapshots(
    session_dir: &Path,
) -> Result<Vec<ArtifactRecord>, AgentRunError> {
    let store = ArtifactStore::new(session_dir.join("artifacts"));
    Ok(vec![
        copy_artifact(&store, session_dir, "manifest.json", "application/json")?,
        copy_artifact(&store, session_dir, "events.jsonl", "application/x-ndjson")?,
    ])
}

fn usage_snapshot(
    manifest: &SessionManifest,
    answers: &[AgentAnswer],
    results: &[ToolResult],
    usage_records: &[UsageRecord],
) -> Value {
    let usage = usage_totals(manifest, usage_records);
    json!({
        "session_id": manifest.id,
        "tokens_limit": manifest.budget.tokens_limit,
        "tokens_used": usage.tokens,
        "cached_tokens": usage.cached_tokens,
        "weighted_credits": usage.weighted_credits,
        "agent_answers": answers.len(),
        "tool_results": results.len()
    })
}

fn limits_snapshot(
    phase: &str,
    manifest: &SessionManifest,
    usage_records: &[UsageRecord],
) -> Value {
    let usage = usage_totals(manifest, usage_records);
    json!({
        "phase": phase,
        "source": "session_budget",
        "tokens_limit": manifest.budget.tokens_limit,
        "tokens_used": usage.tokens,
        "cached_tokens": usage.cached_tokens,
        "weighted_credits": usage.weighted_credits
    })
}

fn usage_totals(manifest: &SessionManifest, usage_records: &[UsageRecord]) -> TokenTotals {
    if usage_records.is_empty() {
        return TokenTotals {
            tokens: manifest.budget.tokens_used,
            cached_tokens: 0,
            weighted_credits: manifest.budget.tokens_used as f64,
        };
    }
    TokenLedger {
        records: usage_records.to_vec(),
    }
    .totals()
}

fn write_json<T: Serialize>(
    store: &ArtifactStore,
    name: &str,
    value: &T,
) -> Result<ArtifactRecord, AgentRunError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AgentRunError::Runtime(error.to_string()))?;
    let text = Redactor::default().redact(&text);
    Ok(store.write_named_text(name, "application/json", &text)?)
}

fn write_diff(store: &ArtifactStore, spec: &TaskSpec) -> Result<ArtifactRecord, AgentRunError> {
    let text = collect_diff(&spec.repo_path);
    let text = OutputBounds::default().apply(&Redactor::default().redact(&text));
    Ok(store.write_named_text("diff.patch", "text/x-diff", &text.text)?)
}

fn write_structured_diff(
    store: &ArtifactStore,
    spec: &TaskSpec,
    required: bool,
) -> Result<(ArtifactRecord, DiffSignal), AgentRunError> {
    if !required {
        return Ok((
            write_json(
                store,
                "diff.json",
                &json!({
                    "status": "not_required",
                    "reason": "current turn does not require diff",
                }),
            )?,
            diff_signal_not_required(),
        ));
    }
    let (value, signal) = match workspace_diff(&spec.repo_path) {
        Ok(diff) => {
            let signal = diff_signal_ok(&diff, required);
            (
                json!({
                    "status": "ok",
                    "diff": diff,
                }),
                signal,
            )
        }
        Err(error) => {
            let signal = diff_signal_unavailable(&error, required);
            (
                json!({
                    "status": "unavailable",
                    "error": error.to_string(),
                }),
                signal,
            )
        }
    };
    Ok((write_json(store, "diff.json", &value)?, signal))
}

fn write_verification(
    store: &ArtifactStore,
    results: &[ToolResult],
    required: bool,
) -> Result<(ArtifactRecord, VerificationSignal), AgentRunError> {
    let value = verification_snapshot(results);
    let signal = verification_signal(&value, required);
    Ok((write_json(store, "verification.json", &value)?, signal))
}

fn verification_snapshot(results: &[ToolResult]) -> Value {
    if let Some(result) = results
        .iter()
        .rev()
        .find(|result| result.name == "run_test")
    {
        if is_denied_result(result) {
            return json!({
                "status": "skipped",
                "source": "approval",
                "command": "run_test",
                "reason": "denied by user",
                "output": bounded_text(&result.text),
            });
        }
        if let Some(snapshot) = structured_test_snapshot(result) {
            return snapshot;
        }
        return json!({
            "status": if result.ok { "passed" } else { "failed" },
            "source": "tool",
            "command": "run_test",
            "output": bounded_text(&result.text),
        });
    }
    json!({
        "status": "skipped",
        "reason": "verification tool was not run",
    })
}

fn results_require_diff(results: &[ToolResult]) -> bool {
    results
        .iter()
        .any(|result| result.ok && result.name == "apply_patch")
}

fn is_denied_result(result: &ToolResult) -> bool {
    !result.ok && result.text == "denied by user"
}

fn structured_test_snapshot(result: &ToolResult) -> Option<Value> {
    let payload = result.json.as_ref()?;
    Some(json!({
        "status": payload.get("status").and_then(Value::as_str).unwrap_or(if result.ok { "passed" } else { "failed" }),
        "source": "tool",
        "command": command_text(payload).unwrap_or_else(|| "run_test".into()),
        "exit_status": payload.get("exit_status").cloned().unwrap_or(Value::Null),
        "output": payload
            .get("output")
            .and_then(Value::as_str)
            .map(bounded_text)
            .unwrap_or_else(|| bounded_text(&result.text)),
        "truncated": payload.get("truncated").and_then(Value::as_bool).unwrap_or(result.truncated),
    }))
}

fn command_text(payload: &Value) -> Option<String> {
    let command = payload.get("command")?.as_array()?;
    let parts = command.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn bounded_text(text: &str) -> String {
    OutputBounds::default()
        .apply(&Redactor::default().redact(text))
        .text
}

fn collect_diff(repo_path: &str) -> String {
    match Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("diff")
        .arg("--no-ext-diff")
        .output()
    {
        Ok(output) => diff_text(output),
        Err(error) => format!("diff_status=unavailable\nreason={error}\n"),
    }
}

fn diff_text(output: Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if text.trim().is_empty() {
        text.push_str("diff_status=clean\n");
    }
    text.push_str(&format!("\nexit_status={}", output.status));
    text
}

fn copy_artifact(
    store: &ArtifactStore,
    session_dir: &Path,
    name: &str,
    mime: &str,
) -> Result<ArtifactRecord, AgentRunError> {
    let text = fs::read_to_string(session_dir.join(name))
        .map_err(|error| AgentRunError::Runtime(format!("failed to read {name}: {error}")))?;
    let text = Redactor::default().redact(&text);
    Ok(store.write_named_text(name, mime, &text)?)
}
