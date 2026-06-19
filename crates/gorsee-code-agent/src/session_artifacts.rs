use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use gorsee_code_artifacts::{ArtifactRecord, ArtifactStore};
use gorsee_code_core::TaskSpec;
use gorsee_code_safety::{OutputBounds, Redactor};
use gorsee_code_session::SessionManifest;
use serde_json::{json, Value};

use crate::{
    protocol::{AgentAnswer, ToolResult},
    report::write_report,
    AgentRunError,
};

pub(crate) fn write_run_artifacts(
    session_dir: &Path,
    manifest: &SessionManifest,
    spec: &TaskSpec,
    skill_id: Option<&str>,
    answers: &[AgentAnswer],
    results: &[ToolResult],
) -> Result<Vec<ArtifactRecord>, AgentRunError> {
    let store = ArtifactStore::new(session_dir.join("artifacts"));
    let mut artifacts = vec![write_report(session_dir, spec, skill_id, answers, results)?];
    artifacts.push(write_json(
        &store,
        "usage.json",
        &usage_snapshot(manifest, answers, results),
    )?);
    artifacts.push(write_json(
        &store,
        "limits-start.json",
        &limits_snapshot("start", manifest),
    )?);
    artifacts.push(write_json(
        &store,
        "limits-end.json",
        &limits_snapshot("end", manifest),
    )?);
    artifacts.push(write_diff(&store, spec)?);
    Ok(artifacts)
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
) -> Value {
    json!({
        "session_id": manifest.id,
        "tokens_limit": manifest.budget.tokens_limit,
        "tokens_used": manifest.budget.tokens_used,
        "agent_answers": answers.len(),
        "tool_results": results.len()
    })
}

fn limits_snapshot(phase: &str, manifest: &SessionManifest) -> Value {
    json!({
        "phase": phase,
        "source": "session_budget",
        "tokens_limit": manifest.budget.tokens_limit,
        "tokens_used": manifest.budget.tokens_used
    })
}

fn write_json(
    store: &ArtifactStore,
    name: &str,
    value: &Value,
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
