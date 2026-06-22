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
        let resolved = self.paths.resolve_existing(&path).map_err(handler)?;
        let text = fs::read_to_string(resolved).map_err(handler)?;
        Ok(ToolOutput {
            text: text.clone(),
            json: Some(json!({ "path": path, "bytes": text.len() })),
            truncated: false,
        })
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

fn arg_path(args: &Value) -> Result<String, ToolRuntimeError> {
    args.get("path")
        .or_else(|| args.get("file"))
        .or_else(|| args.get("file_path"))
        .or_else(|| args.get("uri"))
        .and_then(Value::as_str)
        .map(normalize_path_arg)
        .ok_or_else(|| handler("missing path"))
}

fn normalize_path_arg(path: &str) -> String {
    path.strip_prefix("file://").unwrap_or(path).to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_accepts_file_alias() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("hello.txt"), "hello\n").unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let output = ReadFileTool::new(policy)
            .run(json!({ "file": "hello.txt" }))
            .unwrap();

        assert_eq!(output.text, "hello\n");
    }

    #[test]
    fn read_file_accepts_file_uri_inside_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("hello.txt");
        fs::write(&file, "hello\n").unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let output = ReadFileTool::new(policy)
            .run(json!({ "uri": format!("file://{}", file.display()) }))
            .unwrap();

        assert_eq!(output.text, "hello\n");
    }
}
