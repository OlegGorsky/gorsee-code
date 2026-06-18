use std::{path::PathBuf, process::Command};

use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::Value;

pub struct RunCommandTool {
    root: PathBuf,
}

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
        let output = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&self.root)
            .output()
            .map_err(handler)?;
        Ok(ToolOutput::text(format_output(output)))
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
    if matches!(program, "cargo" | "git" | "npm" | "pnpm" | "pytest" | "go") {
        return Ok(());
    }
    Err(ToolRuntimeError::PermissionDenied(program.into()))
}

fn format_output(output: std::process::Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text.push_str(&format!("\nexit_status={}", output.status));
    text
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}
