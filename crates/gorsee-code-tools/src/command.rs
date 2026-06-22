use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use gorsee_code_safety::{OutputBounds, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

pub struct RunCommandTool {
    root: PathBuf,
}

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

impl RunCommandTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for RunCommandTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "run_command".into(),
            description: "Run an approved allow-listed workspace command".into(),
            risk: RiskClass::Command,
            capabilities: vec!["command:run".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let command = args
            .get("command")
            .and_then(Value::as_array)
            .map(|values| parse_command(values))
            .ok_or_else(|| handler("missing command"))?;
        ensure_allowed(&command)?;
        let output = run_with_timeout(&self.root, &command)?;
        Ok(format_output(&command, output))
    }
}

fn parse_command(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn ensure_allowed(command: &[String]) -> Result<(), ToolRuntimeError> {
    let program = command.first().map(String::as_str).unwrap_or_default();
    if program.trim().is_empty() {
        return Err(ToolRuntimeError::PermissionDenied("empty command".into()));
    }
    match program {
        "cargo" => allow_subcommand(command, &["check", "test", "fmt", "clippy", "build"]),
        "git" => allow_subcommand(
            command,
            &[
                "status",
                "diff",
                "show",
                "log",
                "rev-parse",
                "branch",
                "ls-files",
            ],
        ),
        "npm" | "pnpm" => allow_subcommand(command, &["test", "run", "exec", "--version"]),
        "pytest" => Ok(()),
        "go" => allow_subcommand(command, &["test", "version"]),
        _ => Err(ToolRuntimeError::PermissionDenied(program.into())),
    }
}

fn allow_subcommand(command: &[String], allowed: &[&str]) -> Result<(), ToolRuntimeError> {
    let program = command[0].as_str();
    let subcommand = command.get(1).map(String::as_str).unwrap_or_default();
    if allowed.contains(&subcommand) {
        Ok(())
    } else {
        Err(ToolRuntimeError::PermissionDenied(format!(
            "{program} {subcommand}"
        )))
    }
}

fn run_with_timeout(root: &Path, command: &[String]) -> Result<Output, ToolRuntimeError> {
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(root)
        .spawn()
        .map_err(handler)?;
    let started = Instant::now();
    loop {
        if child.try_wait().map_err(handler)?.is_some() {
            return child.wait_with_output().map_err(handler);
        }
        if started.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(handler(format!(
                "command timed out after {}s",
                COMMAND_TIMEOUT.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn format_output(command: &[String], output: Output) -> ToolOutput {
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.push_str(&format!("\nexit_status={}", output.status));
    let bounded = OutputBounds::default().apply(&text);
    let exit_status = output.status.code();
    let status = if output.status.success() {
        "passed"
    } else {
        "failed"
    };
    ToolOutput {
        text: bounded.text.clone(),
        json: Some(json!({
            "status": status,
            "command": command,
            "exit_status": exit_status,
            "truncated": bounded.truncated,
            "output": bounded.text,
        })),
        truncated: bounded.truncated,
    }
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::process::Output;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    #[test]
    fn empty_command_is_denied_explicitly() {
        let error = ensure_allowed(&[]).unwrap_err();

        assert!(matches!(
            error,
            ToolRuntimeError::PermissionDenied(message) if message == "empty command"
        ));
    }

    #[test]
    fn dangerous_subcommands_are_denied() {
        for command in [cmd(&["git", "push"]), cmd(&["npm", "install"])] {
            assert!(
                matches!(
                    ensure_allowed(&command),
                    Err(ToolRuntimeError::PermissionDenied(_))
                ),
                "{command:?} should be denied"
            );
        }
    }

    #[test]
    fn useful_workspace_commands_are_allowed() {
        for command in [
            cmd(&["cargo", "test"]),
            cmd(&["git", "status"]),
            cmd(&["npm", "run", "test"]),
            cmd(&["pnpm", "run", "test"]),
            cmd(&["go", "test"]),
            cmd(&["pytest"]),
        ] {
            assert!(ensure_allowed(&command).is_ok(), "{command:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn format_output_marks_failed_command_structurally() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(1 << 8),
            stdout: b"nope".to_vec(),
            stderr: Vec::new(),
        };
        let command = vec!["cargo".into(), "check".into()];

        let formatted = format_output(&command, output);

        assert_eq!(formatted.json.as_ref().unwrap()["status"], "failed");
        assert_eq!(formatted.json.as_ref().unwrap()["exit_status"], 1);
        assert_eq!(formatted.json.as_ref().unwrap()["command"][0], "cargo");
        assert!(!formatted.truncated);
    }

    fn cmd(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }
}
