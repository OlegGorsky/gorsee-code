use std::{fs, path::Path};

use gorsee_code_cli::{run_with_options, CliOptions};
use serde_json::Value;

mod support;
use support::assert_product_output;

#[test]
fn route_explains_production_agent_plan_without_live_auth() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "route", "refactor auth and run tests"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("route:"));
    assert!(output.contains("architect glm-5.1"));
    assert!(output.contains("coder deepseek-v4-pro"));
    assert!(output.contains("validator kimi-k2.6"));
    assert_product_output(&output);
}

#[test]
fn budget_set_updates_session_limit_in_project_config() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "budget", "set", "--session", "100k"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();
    let config = fs::read_to_string(temp.path().join("gorsee-code.toml")).unwrap();

    assert!(output.contains("budget: updated"));
    assert!(output.contains("session_tokens=100000"));
    assert!(config.contains("session_tokens = 100000"));
    assert_product_output(&output);
}

#[test]
fn budget_set_updates_agent_limit_in_project_config() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "budget", "set", "--agent", "scout", "10k"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();
    let config = fs::read_to_string(temp.path().join("gorsee-code.toml")).unwrap();

    assert!(output.contains("budget: updated"));
    assert!(output.contains("scout=10000"));
    assert!(config.contains("[agents.scout]"));
    assert!(config.contains("budget_tokens = 10000"));
    assert_product_output(&output);
}

#[test]
fn budget_set_rejects_removed_session_alias() {
    let temp = tempfile::tempdir().unwrap();
    let removed_alias = String::from_utf8(vec![45, 45, 109, 105, 115, 115, 105, 111, 110]).unwrap();

    let error = run_with_options(
        vec![
            "gcode".to_string(),
            "budget".to_string(),
            "set".to_string(),
            removed_alias,
            "100k".to_string(),
        ],
        CliOptions::for_root(temp.path()),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("unexpected argument"));
}

#[test]
fn models_benchmark_summarizes_configured_profiles_without_live_auth() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "models", "benchmark"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("models benchmark: configured"));
    assert!(output.contains("architect glm-5.1"));
    assert!(output.contains("credit_multiplier=1.85"));
    assert!(output.contains("cost=expensive"));
    assert!(output.contains("validator kimi-k2.6"));
    assert_product_output(&output);
}

#[test]
fn models_recommend_selects_coding_profile_from_task() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "models", "recommend", "--task", "frontend bugfix"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("models recommend:"));
    assert!(output.contains("task=frontend bugfix"));
    assert!(output.contains("agent=coder"));
    assert!(output.contains("model=deepseek-v4-pro"));
    assert!(output.contains("reasoning=medium"));
    assert_product_output(&output);
}

#[test]
fn limits_json_reports_missing_auth_without_network() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "limits", "--json"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(value["status"], "skipped");
    assert_eq!(value["reason"], "missing_auth");
    assert_eq!(value["windows"].as_array().unwrap().len(), 0);
    assert_product_output(&output);
}

#[test]
fn limits_watch_once_reports_missing_auth_without_hanging() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "limits", "watch", "--once"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("limits watch:"));
    assert!(output.contains("status=skipped"));
    assert!(output.contains("reason=missing_auth"));
    assert_product_output(&output);
}

#[test]
fn uninstall_keeps_user_data_when_requested() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(["gcode", "init"], CliOptions::for_root(temp.path())).unwrap();

    let output = run_with_options(
        ["gcode", "uninstall", "--user-data", "keep"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("uninstall: complete"));
    assert!(output.contains("user_data=kept"));
    assert!(!temp.path().join("gorsee-code.toml").exists());
    assert!(temp.path().join(".gorsee-code").exists());
    assert_product_output(&output);
}

#[test]
fn uninstall_removes_user_data_when_requested() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(["gcode", "init"], CliOptions::for_root(temp.path())).unwrap();

    let output = run_with_options(
        ["gcode", "uninstall", "--user-data", "remove"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("uninstall: complete"));
    assert!(output.contains("user_data=removed"));
    assert!(!temp.path().join("gorsee-code.toml").exists());
    assert!(!temp.path().join(".gorsee-code").exists());
    assert_product_output(&output);
}

#[test]
fn protect_records_deduplicated_project_paths() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        [
            "gcode",
            "protect",
            "requirements.md",
            "tests/**",
            "requirements.md",
        ],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();
    let config = fs::read_to_string(temp.path().join("gorsee-code.toml")).unwrap();

    assert!(output.contains("protect: updated"));
    assert!(output.contains("paths=2"));
    assert!(config.contains("protected_paths = ["));
    assert!(config.contains("\"requirements.md\""));
    assert!(config.contains("\"tests/**\""));
    assert_product_output(&output);
}

#[test]
fn checkpoint_creates_paused_session_with_event_log() {
    let temp = tempfile::tempdir().unwrap();

    let output =
        run_with_options(["gcode", "checkpoint"], CliOptions::for_root(temp.path())).unwrap();
    let session = only_session(temp.path());
    let manifest = read_manifest(&session);
    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();

    assert!(output.contains("checkpoint:"));
    assert!(output.contains("event: session_paused"));
    assert_eq!(manifest["status"], "paused");
    assert!(events.contains("\"session_paused\""));
    assert_product_output(&output);
}

fn only_session(root: &Path) -> std::path::PathBuf {
    let mut sessions = fs::read_dir(root.join(".gorsee-code").join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    sessions.sort();
    assert_eq!(sessions.len(), 1);
    sessions.pop().unwrap()
}

fn read_manifest(session: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(session.join("manifest.json")).unwrap()).unwrap()
}
