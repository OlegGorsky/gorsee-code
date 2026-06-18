use std::{path::PathBuf, process::Command};

use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::Value;

pub struct GitDiffTool {
    root: PathBuf,
}

pub struct GitStatusTool {
    root: PathBuf,
}

pub struct GitRecentFilesTool {
    root: PathBuf,
}

impl GitDiffTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl GitStatusTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl GitRecentFilesTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for GitDiffTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "git_diff".into(),
            description: "Show git diff for the workspace".into(),
            risk: RiskClass::Read,
            capabilities: vec!["git:diff".into()],
        }
    }

    fn run(&self, _args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .arg("diff")
            .output()
            .map_err(handler)?;
        Ok(ToolOutput::text(
            String::from_utf8_lossy(&output.stdout).to_string(),
        ))
    }
}

impl Tool for GitStatusTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "git_status",
            "Show short git workspace status",
            "git:status",
        )
    }

    fn run(&self, _args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let output = git(&self.root, ["status", "--short"])?;
        Ok(ToolOutput::text(output))
    }
}

impl Tool for GitRecentFilesTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "git_recent_files",
            "List recently changed git files",
            "git:recent",
        )
    }

    fn run(&self, _args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let output = git(
            &self.root,
            ["log", "--name-only", "--pretty=format:", "-n", "20"],
        )?;
        Ok(ToolOutput::text(output))
    }
}

fn git<const N: usize>(root: &PathBuf, args: [&str; N]) -> Result<String, ToolRuntimeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(handler)?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

fn manifest(name: &str, description: &str, capability: &str) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        description: description.into(),
        risk: RiskClass::Read,
        capabilities: vec![capability.into()],
    }
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}
