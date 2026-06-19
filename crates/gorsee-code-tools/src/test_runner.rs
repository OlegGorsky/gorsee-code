use std::{path::PathBuf, process::Command};

use gorsee_code_safety::{OutputBounds, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::Value;

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
            risk: RiskClass::Read,
            capabilities: vec!["tests:run".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let command = args
            .get("command")
            .and_then(Value::as_array)
            .map(|values| command_from_json(values))
            .unwrap_or_else(|| vec!["cargo".into(), "test".into(), "--workspace".into()]);
        ensure_allowed(&command)?;
        let output = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&self.root)
            .output()
            .map_err(handler)?;
        Ok(format_output(output))
    }
}

fn command_from_json(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn ensure_allowed(command: &[String]) -> Result<(), ToolRuntimeError> {
    let program = command.first().map(String::as_str).unwrap_or_default();
    if matches!(program, "cargo" | "npm" | "pnpm" | "yarn" | "pytest" | "go") {
        Ok(())
    } else {
        Err(ToolRuntimeError::PermissionDenied(program.into()))
    }
}

fn format_output(output: std::process::Output) -> ToolOutput {
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.push_str(&format!("\nexit_status={}", output.status));
    let bounded = OutputBounds::default().apply(&text);
    ToolOutput {
        text: bounded.text,
        json: None,
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

    #[cfg(unix)]
    #[test]
    fn format_output_marks_long_test_output_as_truncated() {
        let output = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![b'a'; OutputBounds::default().max_bytes + 1024],
            stderr: Vec::new(),
        };

        let formatted = format_output(output);

        assert!(formatted.truncated);
        assert!(formatted.text.contains("output truncated"));
    }
}
