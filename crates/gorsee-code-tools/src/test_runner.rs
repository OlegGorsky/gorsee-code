use std::{fs, path::Path, path::PathBuf, process::Command};

use gorsee_code_safety::{OutputBounds, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

pub struct RunTestTool {
    root: PathBuf,
}

impl RunTestTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for RunTestTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "run_test".into(),
            description: "Run an allow-listed test command".into(),
            risk: RiskClass::Command,
            capabilities: vec!["tests:run".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let command = test_command_from_args(&args).unwrap_or_else(|| default_command(&self.root));
        if command.is_empty() {
            return Ok(skipped_output("no supported verification command detected"));
        }
        if !test_command_allowed(&command) {
            return Ok(skipped_output(&format!(
                "unsupported verification command: {}",
                command.first().map(String::as_str).unwrap_or_default()
            )));
        }
        if !test_command_supported_in_workspace(&self.root, &command) {
            return Ok(skipped_output(&format!(
                "no supported verification command detected for {}",
                command.first().map(String::as_str).unwrap_or_default()
            )));
        }
        let output = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&self.root)
            .output()
            .map_err(handler)?;
        Ok(format_output(&command, output))
    }
}

pub fn default_test_command(root: &Path) -> Vec<String> {
    default_command(root)
}

fn default_command(root: &Path) -> Vec<String> {
    if root.join("Cargo.toml").exists()
        && (root.join("src").exists() || root.join("tests").exists())
    {
        return vec!["cargo".into(), "test".into(), "--workspace".into()];
    }
    if let Some(command) = node_test_command(root) {
        return command;
    }
    if root.join("pyproject.toml").exists() || root.join("pytest.ini").exists() {
        return vec!["pytest".into()];
    }
    if root.join("go.mod").exists() {
        return vec!["go".into(), "test".into(), "./...".into()];
    }
    Vec::new()
}

fn node_test_command(root: &Path) -> Option<Vec<String>> {
    let package_json = root.join("package.json");
    if !package_json.exists() {
        return None;
    }
    let value = serde_json::from_str::<Value>(&fs::read_to_string(package_json).ok()?).ok()?;
    let scripts = value.get("scripts").and_then(Value::as_object)?;
    if !scripts.contains_key("test") {
        return None;
    }
    if root.join("pnpm-lock.yaml").exists() {
        return Some(vec!["pnpm".into(), "test".into(), "--if-present".into()]);
    }
    Some(vec!["npm".into(), "test".into(), "--if-present".into()])
}

pub fn test_command_from_args(args: &Value) -> Option<Vec<String>> {
    let value = args.get("command")?;
    match value {
        Value::Array(values) => Some(command_from_json(values)),
        Value::String(command) => Some(command_from_string(command)),
        _ => None,
    }
}

fn command_from_json(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|part| !part.trim().is_empty())
        .collect()
}

fn command_from_string(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(ToOwned::to_owned)
        .filter(|part| !part.trim().is_empty())
        .collect()
}

pub fn test_command_allowed(command: &[String]) -> bool {
    let program = command.first().map(String::as_str).unwrap_or_default();
    matches!(program, "cargo" | "npm" | "pnpm" | "yarn" | "pytest" | "go")
}

pub fn test_command_supported_in_workspace(root: &Path, command: &[String]) -> bool {
    match command.first().map(String::as_str).unwrap_or_default() {
        "cargo" => root.join("Cargo.toml").exists(),
        "npm" | "pnpm" | "yarn" => root.join("package.json").exists(),
        "pytest" => {
            root.join("pyproject.toml").exists()
                || root.join("pytest.ini").exists()
                || root.join("tests").is_dir()
        }
        "go" => root.join("go.mod").exists(),
        _ => false,
    }
}

fn format_output(command: &[String], output: std::process::Output) -> ToolOutput {
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

fn skipped_output(reason: &str) -> ToolOutput {
    ToolOutput {
        text: reason.into(),
        json: Some(json!({
            "status": "skipped",
            "reason": reason,
            "command": [],
            "truncated": false,
            "output": reason,
        })),
        truncated: false,
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

    #[cfg(unix)]
    #[test]
    fn format_output_marks_long_test_output_as_truncated() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![b'a'; OutputBounds::default().max_bytes + 1024],
            stderr: Vec::new(),
        };

        let command = vec!["cargo".into(), "test".into()];
        let formatted = format_output(&command, output);

        assert!(formatted.truncated);
        assert!(formatted.text.contains("output truncated"));
    }

    #[cfg(unix)]
    #[test]
    fn format_output_marks_failed_command_structurally() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(1 << 8),
            stdout: b"fail".to_vec(),
            stderr: Vec::new(),
        };
        let command = vec!["cargo".into(), "test".into()];

        let formatted = format_output(&command, output);

        assert_eq!(formatted.json.as_ref().unwrap()["status"], "failed");
        assert_eq!(formatted.json.as_ref().unwrap()["exit_status"], 1);
    }

    #[test]
    fn default_command_detects_node_test_script() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts":{"test":"echo ok"}}"#,
        )
        .unwrap();

        assert_eq!(
            default_command(temp.path()),
            vec![
                "npm".to_string(),
                "test".to_string(),
                "--if-present".to_string()
            ]
        );
    }

    #[test]
    fn default_command_skips_unknown_workspace() {
        let temp = tempfile::tempdir().unwrap();

        let output = skipped_output("no supported verification command detected");

        assert!(default_command(temp.path()).is_empty());
        assert_eq!(output.json.as_ref().unwrap()["status"], "skipped");
    }

    #[test]
    fn run_test_accepts_string_command() {
        let command =
            test_command_from_args(&json!({ "command": "cargo test --workspace" })).unwrap();

        assert_eq!(command, vec!["cargo", "test", "--workspace"]);
    }

    #[test]
    fn run_test_rejects_non_test_programs() {
        let output = RunTestTool::new(PathBuf::from("."))
            .run(json!({ "command": "cat hello.txt" }))
            .unwrap();

        assert_eq!(output.json.as_ref().unwrap()["status"], "skipped");
        assert!(output.text.contains("unsupported verification command"));
    }

    #[test]
    fn run_test_skips_allowed_program_without_project_marker() {
        let temp = tempfile::tempdir().unwrap();

        let output = RunTestTool::new(temp.path().to_path_buf())
            .run(json!({ "command": "pytest" }))
            .unwrap();

        assert_eq!(output.json.as_ref().unwrap()["status"], "skipped");
        assert!(output.text.contains("no supported verification command"));
    }

    #[test]
    fn run_test_is_command_risk() {
        let manifest = RunTestTool::new(PathBuf::from(".")).manifest();

        assert_eq!(manifest.risk, RiskClass::Command);
    }

    #[test]
    fn registry_requires_approval_for_run_test() {
        let temp = tempfile::tempdir().unwrap();
        let registry = crate::builtin_registry(temp.path()).unwrap();

        let approval = registry.approval_required("run_test").unwrap();

        assert_eq!(
            approval.map(|manifest| manifest.risk),
            Some(RiskClass::Command)
        );
    }
}
