use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use gorsee_code_cli::{auth, run_with_options, CliOptions};
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
fn skills_list_contains_builtin_presets() {
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
fn skills_run_without_auth_reports_missing_auth_and_creates_no_session() {
    let temp = tempfile::tempdir().unwrap();
    let error = run_with_options(
        ["gcode", "skills", "run", "repo-audit"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("missing_auth"));
    assert_no_sessions(temp.path());
}

#[test]
fn pause_marks_latest_session_and_appends_event() {
    let temp = tempfile::tempdir().unwrap();
    create_session(temp.path(), "2026-06-19T00-00-00_pause-test");

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
fn gcode_prompts_for_key_then_asks_for_mission_objective() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .current_dir(temp.path())
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"ng_sk_test_123456\nq\n").unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("NeuroGate API key:"));
    assert!(stdout.contains("Mission objective:"));
    assert_product_output(&stdout);
    assert!(stdout.contains("\x1b[?1049h"));
    assert!(stdout.contains("\x1b[?1049l"));
    assert!(auth::status(temp.path(), None).unwrap().configured);
}

#[test]
fn mission_without_auth_reports_missing_auth_and_creates_no_session() {
    let temp = tempfile::tempdir().unwrap();
    let error = run_with_options(
        ["gcode", "mission", "audit this repository"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("missing_auth"));
    assert_no_sessions(temp.path());
}

#[test]
fn usage_and_capabilities_are_product_ready_without_auth() {
    let temp = tempfile::tempdir().unwrap();

    let usage = run_with_options(["gcode", "usage"], CliOptions::for_root(temp.path())).unwrap();
    let capabilities =
        run_with_options(["gcode", "capabilities"], CliOptions::for_root(temp.path())).unwrap();

    assert!(usage.contains("usage: current"));
    assert!(capabilities.contains("capabilities: configured"));
    assert_product_output(&usage);
    assert_product_output(&capabilities);
}

fn assert_product_output(output: &str) {
    let lowered = output.to_lowercase();
    for forbidden in [
        word(&['f', 'o', 'u', 'n', 'd', 'a', 't', 'i', 'o', 'n']),
        word(&[
            'v', 'e', 'r', 't', 'i', 'c', 'a', 'l', ' ', 's', 'l', 'i', 'c', 'e',
        ]),
        word(&['f', 'i', 'x', 't', 'u', 'r', 'e']),
        word(&['s', 'c', 'a', 'f', 'f', 'o', 'l', 'd']),
        word(&['m', 'v', 'p']),
        word(&['m', 'i', 'n', 'i', 'm', 'a', 'l']),
        word(&['d', 'e', 'm', 'o']),
        word(&['p', 'l', 'a', 'c', 'e', 'h', 'o', 'l', 'd', 'e', 'r']),
    ] {
        assert!(
            !lowered.contains(&forbidden),
            "output contained forbidden product wording {forbidden:?}: {output}"
        );
    }
}

fn word(chars: &[char]) -> String {
    chars.iter().collect()
}

fn only_session(root: &Path) -> PathBuf {
    let sessions = session_dirs(root);
    assert_eq!(sessions.len(), 1);
    sessions.into_iter().next().unwrap()
}

fn session_dirs(root: &Path) -> Vec<PathBuf> {
    let path = root.join(".gorsee-code").join("sessions");
    let mut sessions = match fs::read_dir(path) {
        Ok(entries) => entries
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("read sessions directory: {error}"),
    };
    sessions.sort();
    sessions
}

fn assert_no_sessions(root: &Path) {
    assert!(session_dirs(root).is_empty());
}

fn create_session(root: &Path, id: &str) {
    let session = root.join(".gorsee-code").join("sessions").join(id);
    fs::create_dir_all(session.join("artifacts")).unwrap();
    fs::write(
        session.join("manifest.json"),
        serde_json::json!({
            "id": id,
            "repo": root.display().to_string(),
            "branch": "main",
            "started_at": "2026-06-19T00:00:00Z",
            "status": "running",
            "agents": ["architect", "scout", "coder", "validator"],
            "budget": { "tokens_limit": 80000, "tokens_used": 0 }
        })
        .to_string(),
    )
    .unwrap();
}

fn read_manifest(session: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(session.join("manifest.json")).unwrap()).unwrap()
}
