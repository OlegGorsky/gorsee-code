use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use gorsee_code_cli::{auth, run_with_options, CliOptions};
use gorsee_code_gateway::GatewayState;
use serde_json::Value;

#[test]
fn init_creates_project_config_without_secret() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_with_options(["gcode", "init"], CliOptions::for_root(temp.path())).unwrap();

    let config = temp.path().join("gorsee-code.toml");
    let local = temp.path().join(".gorsee-code");
    let text = std::fs::read_to_string(config).unwrap();

    assert!(output.contains("initialized"));
    assert!(local.join("sessions").is_dir());
    assert!(text.contains("[neurogate]"));
    assert!(!text.contains("api_key"));
}

#[test]
fn auth_set_and_status_redact_stored_key() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(
        ["gcode", "auth", "set", "ng_sk_test_123456"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    let status = auth::status(temp.path(), None).unwrap();
    let output = run_with_options(
        ["gcode", "auth", "status"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(status.configured);
    assert!(output.contains("local_file"));
    assert!(output.contains("ng_s"));
    assert!(output.contains("3456"));
    assert!(!output.contains("ng_sk_test_123456"));
}

#[test]
fn auth_set_reads_env_key_when_argument_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = CliOptions::for_root(temp.path());
    options.env_key = Some("ng_sk_env_123456".into());

    let output = run_with_options(["gcode", "auth", "set"], options).unwrap();
    let status = auth::status(temp.path(), None).unwrap();

    assert!(status.configured);
    assert!(output.contains("local_file"));
    assert!(output.contains("ng_s"));
    assert!(output.contains("3456"));
    assert!(!output.contains("ng_sk_env_123456"));
}

#[test]
fn doctor_without_key_reports_local_checks_and_skips_live_api() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(["gcode", "init"], CliOptions::for_root(temp.path())).unwrap();

    let output = run_with_options(["gcode", "doctor"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.contains("config: ok"));
    assert!(output.contains("auth: missing"));
    assert!(output.contains("neurogate: skipped"));
}

#[test]
fn skills_list_contains_foundation_presets() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_with_options(
        ["gcode", "skills", "list"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("repo-audit"));
    assert!(output.contains("bug-fix"));
    assert!(output.contains("release-check"));
}

#[test]
fn skills_run_creates_session_artifact_and_gateway_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_with_options(
        ["gcode", "skills", "run", "repo-audit"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    let session = only_session(temp.path());
    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();
    let artifacts = artifact_paths(&session);
    let state = GatewayState::fixture(temp.path());

    assert!(output.contains("skill: repo-audit"));
    assert!(output.contains("artifacts=1"));
    assert!(events.contains("\"skill_started\""));
    assert!(events.contains("\"artifact_created\""));
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0].extension().and_then(|value| value.to_str()),
        Some("md")
    );
    assert_eq!(state.artifacts.len(), 1);
    assert_eq!(state.artifacts[0].mime, "text/markdown");
}

#[test]
fn pause_marks_latest_session_and_appends_event() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(
        ["gcode", "mission", "audit this repository"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    let output = run_with_options(["gcode", "pause"], CliOptions::for_root(temp.path())).unwrap();
    let session = only_session(temp.path());
    let manifest = read_manifest(&session);
    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();

    assert!(output.contains("event: mission_paused"));
    assert_eq!(manifest["status"], "paused");
    assert!(events.contains("\"mission_paused\""));
}

#[test]
fn help_is_success_output() {
    let temp = tempfile::tempdir().unwrap();
    let output = run_with_options(["gcode", "--help"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.contains("Usage: gcode"));
    assert!(output.contains("Commands:"));
}

#[test]
fn version_is_success_output() {
    let temp = tempfile::tempdir().unwrap();
    let output =
        run_with_options(["gcode", "--version"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.starts_with("gcode "));
}

#[test]
fn gcode_prompts_for_key_then_opens_tui() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .current_dir(temp.path())
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"ng_sk_test_123456\n")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("NeuroGate API key:"));
    assert!(stdout.contains("Gorsee Code Mission"));
    assert!(auth::status(temp.path(), None).unwrap().configured);
}

fn only_session(root: &Path) -> PathBuf {
    let sessions = session_dirs(root);
    assert_eq!(sessions.len(), 1);
    sessions.into_iter().next().unwrap()
}

fn session_dirs(root: &Path) -> Vec<PathBuf> {
    let mut sessions = fs::read_dir(root.join(".gorsee-code").join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    sessions.sort();
    sessions
}

fn artifact_paths(session: &Path) -> Vec<PathBuf> {
    let mut artifacts = fs::read_dir(session.join("artifacts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    artifacts.sort();
    artifacts
}

fn read_manifest(session: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(session.join("manifest.json")).unwrap()).unwrap()
}
