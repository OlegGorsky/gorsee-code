use std::{
    io::Write,
    process::{Command, Stdio},
};

use gorsee_code_cli::auth;

mod support;
use support::{assert_product_output, PtySession};

#[test]
fn gcode_prompts_for_key_then_opens_workspace_app() {
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
    assert!(stdout.contains("Введите задачу"));
    assert!(stdout.contains("Enter запуск"));
    let old_label = ["Ta", "sk:"].concat();
    assert!(!stdout.contains(&old_label));
    assert_product_output(&stdout);
    assert!(stdout.contains("\x1b[?1049h"));
    assert!(stdout.contains("\x1b[?1049l"));
    assert!(auth::status(temp.path(), None).unwrap().configured);
}

#[test]
fn gcode_accepts_workspace_commands_after_key_prompt() {
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
    stdin.write_all(b"ng_sk_test_123456\n/agents\nq\n").unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("NeuroGate API key:"));
    assert!(stdout.contains("Введите задачу"));
    assert!(stdout.contains("Результат команды"));
    assert!(stdout.contains("agents:"));
    assert_product_output(&stdout);
}

#[test]
fn gcode_tui_subcommand_opens_workspace_app() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("tui")
        .current_dir(temp.path())
        .env("NEUROGATE_API_KEY", "ng_sk_test_123456")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"q\n").unwrap();
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("Введите задачу"));
    assert!(stdout.contains("Gorsee Code"));
    assert!(!stdout.contains("Gorsee Code Workspace\nСессия"));
    assert_product_output(&stdout);
}

#[test]
fn gcode_hides_key_when_prompt_runs_in_tty() {
    let temp = tempfile::tempdir().unwrap();
    let mut session = PtySession::spawn_gcode(env!("CARGO_BIN_EXE_gcode"), temp.path());

    assert!(session.wait_for("NeuroGate API key:"));
    session.send("ng_sk_tty_123456\r");
    assert!(session.wait_for("Введите задачу"));
    session.send("q");

    let (status, transcript) = session.finish();

    assert!(status.success());
    assert!(transcript.contains("NeuroGate API key:"));
    assert!(!transcript.contains("ng_sk_tty_123456"));
    assert!(auth::status(temp.path(), None).unwrap().configured);
}
