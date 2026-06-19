use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

use gorsee_code_cli::{run_with_options, CliOptions};
use serde_json::Value;

mod support;
use support::assert_product_output;

#[test]
fn setup_prepares_workspace_and_stores_env_key_without_manual_next_steps() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = CliOptions::for_root(temp.path());
    options.env_key = Some("ng_sk_setup_123456".into());

    let output = run_with_options(["gcode", "setup"], options).unwrap();

    assert!(temp.path().join("gorsee-code.toml").is_file());
    assert!(temp.path().join(".gorsee-code").join("sessions").is_dir());
    assert!(temp.path().join(".gorsee-code").join("auth.json").is_file());
    assert!(output.contains("setup: ready"));
    assert!(output.contains("auth: configured"));
    assert!(!output.contains("next:"));
    assert!(!output.contains("ng_sk_setup_123456"));
    assert_product_output(&output);
}

#[cfg(unix)]
#[test]
fn auth_file_is_written_with_private_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();

    run_with_options(
        ["gcode", "auth", "set", "ng_sk_private_123456"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    let mode = fs::metadata(temp.path().join(".gorsee-code").join("auth.json"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(mode, 0o600);
}

#[test]
fn resume_marks_session_running_and_appends_resume_event() {
    let temp = tempfile::tempdir().unwrap();
    create_session(temp.path(), "2026-06-19T00-00-00_resume-test", "paused");

    let output = run_with_options(["gcode", "resume"], CliOptions::for_root(temp.path())).unwrap();
    let session = only_session(temp.path());
    let manifest = read_manifest(&session);
    let events = fs::read_to_string(session.join("events.jsonl")).unwrap();

    assert!(output.contains("resume: 2026-06-19T00-00-00_resume-test"));
    assert!(output.contains("event: session_resumed"));
    assert_eq!(manifest["status"], "running");
    assert!(events.contains("\"session_resumed\""));
    assert_product_output(&output);
}

#[test]
fn models_show_credit_multipliers_for_configured_matrix() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(["gcode", "models"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.contains("models: configured"));
    assert!(output.contains("credit_multiplier="));
    assert!(output.contains("neurogate/gpt-5"));
    assert_product_output(&output);
}

#[test]
fn production_discovery_commands_work_without_live_auth() {
    let temp = tempfile::tempdir().unwrap();

    for (args, expected) in [
        (vec!["gcode", "capabilities"], "capabilities: configured"),
        (
            vec!["gcode", "doctor"],
            "neurogate: skipped reason=missing_auth",
        ),
        (vec!["gcode", "hooks"], "hooks:"),
        (vec!["gcode", "skills", "list"], "skills:"),
    ] {
        let output = run_with_options(args, CliOptions::for_root(temp.path())).unwrap();

        assert!(output.contains(expected));
        assert_product_output(&output);
    }
}

#[test]
fn files_command_lists_workspace_files_without_live_auth() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("main.rs"), "fn main() {}\n").unwrap();

    let output = run_with_options(["gcode", "files"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.contains("files:"));
    assert!(output.contains("src/main.rs"));
    assert_product_output(&output);
}

#[test]
fn diff_command_shows_current_workspace_diff_without_live_auth() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), ["init"]);
    fs::write(temp.path().join("tracked.txt"), "original\n").unwrap();
    git(temp.path(), ["add", "tracked.txt"]);
    fs::write(temp.path().join("tracked.txt"), "changed\n").unwrap();

    let output = run_with_options(["gcode", "diff"], CliOptions::for_root(temp.path())).unwrap();

    assert!(output.contains("diff:"));
    assert!(output.contains("tracked.txt"));
    assert!(output.contains("+changed"));
    assert_product_output(&output);
}

#[test]
fn reset_yes_removes_project_data() {
    let temp = tempfile::tempdir().unwrap();
    run_with_options(["gcode", "init"], CliOptions::for_root(temp.path())).unwrap();
    create_session(temp.path(), "2026-06-19T00-00-00_reset-test", "running");

    let output = run_with_options(
        ["gcode", "reset", "--yes"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("reset: complete"));
    assert!(!temp.path().join(".gorsee-code").exists());
    assert!(!temp.path().join("gorsee-code.toml").exists());
    assert_product_output(&output);
}

#[test]
fn approval_commands_list_pending_items_without_auth() {
    let temp = tempfile::tempdir().unwrap();
    let old_id = "z-old-approval-test";
    let old = create_session_at(
        temp.path(),
        old_id,
        "waiting_approval",
        "2026-06-19T00:00:00Z",
    );
    write_pending_approval(&old, old_id, "appr_old");
    let new_id = "a-new-approval-test";
    let new = create_session_at(
        temp.path(),
        new_id,
        "waiting_approval",
        "2026-06-19T01:00:00Z",
    );
    write_pending_approval(&new, new_id, "appr_new");

    let list = run_with_options(["gcode", "approvals"], CliOptions::for_root(temp.path())).unwrap();
    assert!(list.contains("approvals: pending"));
    assert!(list.contains("appr_new"));
    assert!(!list.contains("appr_old"));
    assert!(list.contains("propose_patch"));
    assert!(list.contains("risk=write"));
    assert_product_output(&list);
}

#[test]
fn approval_command_rejects_pending_item_without_saved_execution() {
    let temp = tempfile::tempdir().unwrap();
    let session_id = "2026-06-19T00-00-00_approval-test";
    let session = create_session(temp.path(), session_id, "waiting_approval");
    write_pending_approval(&session, session_id, "appr_0001");

    let error = run_with_options(
        ["gcode", "approve", "appr_0001"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("saved execution is missing"));
    assert!(!error.contains("missing_auth"));
    assert_product_output(&error);
}

#[test]
fn approve_does_not_execute_write_tool_without_saved_execution() {
    let temp = tempfile::tempdir().unwrap();
    let session_id = "2026-06-19T00-00-00_apply-test";
    let session = create_session(temp.path(), session_id, "waiting_approval");
    write_apply_patch_approval(&session, session_id, "appr_0001");

    let error = run_with_options(
        ["gcode", "approve", "appr_0001"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap_err()
    .to_string();
    let approvals = fs::read_to_string(session.join("approvals.jsonl")).unwrap();

    assert!(error.contains("saved execution is missing"));
    assert!(approvals.contains("\"status\":\"pending\""));
    let target = temp.path().join("src").join("lib.rs");

    assert!(!target.exists());
    assert_product_output(&error);
}

fn create_session(root: &Path, id: &str, status: &str) -> PathBuf {
    create_session_at(root, id, status, "2026-06-19T00:00:00Z")
}

fn create_session_at(root: &Path, id: &str, status: &str, started_at: &str) -> PathBuf {
    let session = root.join(".gorsee-code").join("sessions").join(id);
    fs::create_dir_all(session.join("artifacts")).unwrap();
    fs::create_dir_all(session.join("patches")).unwrap();
    fs::write(session.join("events.jsonl"), "").unwrap();
    fs::write(
        session.join("manifest.json"),
        serde_json::json!({
            "id": id,
            "repo": root.display().to_string(),
            "branch": "main",
            "started_at": started_at,
            "status": status,
            "agents": ["architect", "scout", "coder", "validator"],
            "budget": { "tokens_limit": 80000, "tokens_used": 0 }
        })
        .to_string(),
    )
    .unwrap();
    session
}

fn write_pending_approval(session: &Path, session_id: &str, id: &str) {
    let record = serde_json::json!({
        "id": id,
        "session_id": session_id,
        "sequence": 2,
        "agent_id": "coder",
        "tool_name": "propose_patch",
        "args": { "path": "src/lib.rs", "content": "pub fn ready() {}\n" },
        "risk": "write",
        "status": "pending",
        "created_at": "2026-06-19T00:00:00Z",
        "decided_at": null
    });
    fs::write(session.join("approvals.jsonl"), format!("{record}\n")).unwrap();
}

fn write_apply_patch_approval(session: &Path, session_id: &str, id: &str) {
    let record = serde_json::json!({
        "id": id,
        "session_id": session_id,
        "sequence": 2,
        "agent_id": "coder",
        "tool_name": "apply_patch",
        "args": { "path": "src/lib.rs", "content": "pub fn ready() {}\n" },
        "risk": "write",
        "status": "pending",
        "created_at": "2026-06-19T00:00:00Z",
        "decided_at": null
    });
    fs::write(session.join("approvals.jsonl"), format!("{record}\n")).unwrap();
}

fn only_session(root: &Path) -> PathBuf {
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

fn git<const N: usize>(root: &Path, args: [&str; N]) {
    let status = StdCommand::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}
