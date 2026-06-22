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
        .env("GORSEE_CODE_AUTH_HOME", temp.path().join("global-auth"))
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
        .env("GORSEE_CODE_AUTH_HOME", temp.path().join("global-auth"))
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
fn prompted_key_is_available_from_another_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let first_root = temp.path().join("first");
    let second_root = temp.path().join("second");
    let global_auth = temp.path().join("global-auth");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .current_dir(&first_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", &global_auth)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"ng_sk_global_123456\nq\n").unwrap();
    drop(stdin);
    assert!(child.wait_with_output().unwrap().status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("auth")
        .arg("status")
        .current_dir(second_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", global_auth)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("auth: configured source=global_file"));
    assert!(stdout.contains("ng_s"));
    assert!(!stdout.contains("ng_sk_global_123456"));
}

#[test]
fn auth_set_key_is_available_from_another_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let first_root = temp.path().join("first");
    let second_root = temp.path().join("second");
    let global_auth = temp.path().join("global-auth");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();

    let set_output = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("auth")
        .arg("set")
        .arg("ng_sk_auth_set_123456")
        .current_dir(&first_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", &global_auth)
        .output()
        .unwrap();

    assert!(set_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("auth")
        .arg("status")
        .current_dir(second_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", global_auth)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("auth: configured source=global_file"));
    assert!(stdout.contains("ng_s"));
    assert!(!stdout.contains("ng_sk_auth_set_123456"));
}

#[test]
fn existing_workspace_key_is_mirrored_to_global_auth_on_tui_start() {
    let temp = tempfile::tempdir().unwrap();
    let first_root = temp.path().join("first");
    let second_root = temp.path().join("second");
    let global_auth = temp.path().join("global-auth");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();
    auth::set(&first_root, "ng_sk_local_123456").unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .current_dir(&first_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", &global_auth)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"q\n").unwrap();
    drop(stdin);
    let first_output = child.wait_with_output().unwrap();

    assert!(first_output.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("auth")
        .arg("status")
        .current_dir(second_root)
        .env_remove("NEUROGATE_API_KEY")
        .env_remove("GORSEE_NEUROGATE_API_KEY")
        .env("GORSEE_CODE_AUTH_HOME", global_auth)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("auth: configured source=global_file"));
    assert!(!stdout.contains("ng_sk_local_123456"));
}

#[test]
fn gcode_tui_subcommand_opens_workspace_app() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gcode"))
        .arg("tui")
        .current_dir(temp.path())
        .env("NEUROGATE_API_KEY", "ng_sk_test_123456")
        .env("GORSEE_CODE_AUTH_HOME", temp.path().join("global-auth"))
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
