use std::{fs, path::PathBuf};

use gorsee_code_safety::{PathPolicy, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use ignore::WalkBuilder;
use serde_json::{json, Value};

pub struct ListFilesTool {
    root: PathBuf,
}

impl ListFilesTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for ListFilesTool {
    fn manifest(&self) -> ToolManifest {
        manifest("list_files", "List workspace files", vec!["files:list"])
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let max = args.get("max").and_then(Value::as_u64).unwrap_or(200) as usize;
        let files: Vec<_> = WalkBuilder::new(&self.root)
            .hidden(false)
            .build()
            .flatten()
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .take(max)
            .map(|entry| relative(&self.root, entry.path()))
            .collect();
        Ok(ToolOutput {
            text: files.join("\n"),
            json: Some(json!({ "files": files })),
            truncated: false,
        })
    }
}

pub struct ReadFileTool {
    paths: PathPolicy,
}

impl ReadFileTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl Tool for ReadFileTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "read_file",
            "Read a UTF-8 workspace file",
            vec!["files:read"],
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let path = arg_path(&args)?;
        let path = self.paths.resolve_existing(path).map_err(handler)?;
        let text = fs::read_to_string(path).map_err(handler)?;
        Ok(ToolOutput::text(text))
    }
}

pub struct SearchTextTool {
    paths: PathPolicy,
}

impl SearchTextTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl Tool for SearchTextTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "search_text",
            "Search text in workspace files",
            vec!["files:search"],
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| handler("missing query"))?;
        let max = args.get("max").and_then(Value::as_u64).unwrap_or(100) as usize;
        let root = self.paths.root().to_path_buf();
        let mut matches = Vec::new();
        for entry in WalkBuilder::new(&root).hidden(false).build().flatten() {
            if matches.len() >= max || !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            collect_matches(&root, entry.path().to_path_buf(), query, &mut matches);
        }
        Ok(ToolOutput {
            text: matches.join("\n"),
            json: Some(json!({ "matches": matches })),
            truncated: false,
        })
    }
}

fn collect_matches(root: &std::path::Path, path: PathBuf, query: &str, matches: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(&path) else {
        return;
    };
    let needle = query.to_lowercase();
    for (index, line) in text.lines().enumerate() {
        if line.to_lowercase().contains(&needle) {
            matches.push(format!(
                "{}:{}:{}",
                relative(root, &path),
                index + 1,
                line.trim()
            ));
        }
    }
}

fn manifest(name: &str, description: &str, capabilities: Vec<&str>) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        description: description.into(),
        risk: RiskClass::Read,
        capabilities: capabilities.into_iter().map(str::to_string).collect(),
    }
}

fn arg_path(args: &Value) -> Result<&str, ToolRuntimeError> {
    args.get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| handler("missing path"))
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}

fn relative(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
