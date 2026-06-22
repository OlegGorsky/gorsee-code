use std::{path::PathBuf, process::Command};

use gorsee_code_diff::workspace_diff;
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

pub struct GitDiffTool {
    root: PathBuf,
}

pub struct GitStatusTool {
    root: PathBuf,
}

pub struct GitRecentFilesTool {
    root: PathBuf,
}

pub struct GitChangedFilesTool {
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

impl GitChangedFilesTool {
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
        match workspace_diff(&self.root) {
            Ok(diff) => {
                let text = diff.render_summary_text();
                Ok(ToolOutput {
                    text,
                    json: Some(json!({ "diff": diff })),
                    truncated: false,
                })
            }
            Err(_) => git(&self.root, ["diff"]),
        }
    }
}

impl Tool for GitChangedFilesTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "git_changed_files",
            "List changed git files from structured diff state",
            "git:changed_files",
        )
    }

    fn run(&self, _args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        match workspace_diff(&self.root) {
            Ok(diff) => {
                let files = diff.files.iter().map(|file| &file.path).collect::<Vec<_>>();
                Ok(ToolOutput {
                    text: diff.changed_files_text(),
                    json: Some(json!({ "files": files, "summary": diff.summary })),
                    truncated: false,
                })
            }
            Err(_) => git(&self.root, ["status", "--short"]),
        }
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
        git(&self.root, ["status", "--short"])
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
        git(
            &self.root,
            ["log", "--name-only", "--pretty=format:", "-n", "20"],
        )
    }
}

fn git<const N: usize>(root: &PathBuf, args: [&str; N]) -> Result<ToolOutput, ToolRuntimeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(handler)?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        return Ok(ToolOutput::text(text));
    }
    let summary = human_git_error(&text);
    Ok(ToolOutput {
        text: summary.clone(),
        json: Some(json!({
            "status": "unavailable",
            "output": summary,
            "exit_status": output.status.code(),
        })),
        truncated: false,
    })
}

fn human_git_error(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if lower.contains("not a git repository") {
        "git недоступен: рабочая папка не является git-репозиторием".into()
    } else {
        text.lines().next().unwrap_or("git недоступен").to_string()
    }
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

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command};

    use gorsee_code_tool_runtime::Tool;

    use super::*;

    #[test]
    fn git_diff_tool_exposes_structured_hunks() {
        let temp = tempfile::tempdir().unwrap();
        git_cmd(temp.path(), ["init"]);
        fs::write(temp.path().join("tracked.txt"), "before\n").unwrap();
        git_cmd(temp.path(), ["add", "tracked.txt"]);
        fs::write(temp.path().join("tracked.txt"), "after\n").unwrap();

        let output = GitDiffTool::new(temp.path().to_path_buf())
            .run(serde_json::json!({}))
            .unwrap();

        assert_eq!(
            output.json.as_ref().unwrap()["diff"]["files"][0]["hunks"][0]["lines"][0]["kind"],
            "delete"
        );
        assert_eq!(
            output.json.as_ref().unwrap()["diff"]["files"][0]["hunks"][0]["lines"][1]["kind"],
            "insert"
        );
    }

    #[test]
    fn git_tools_report_non_git_workspace_concisely() {
        let temp = tempfile::tempdir().unwrap();

        let output = GitDiffTool::new(temp.path().to_path_buf())
            .run(serde_json::json!({}))
            .unwrap();

        assert_eq!(output.json.as_ref().unwrap()["status"], "unavailable");
        assert!(output.text.contains("не является git-репозиторием"));
        assert!(!output.text.contains("Diff output format options"));
    }

    fn git_cmd<const N: usize>(root: &Path, args: [&str; N]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
