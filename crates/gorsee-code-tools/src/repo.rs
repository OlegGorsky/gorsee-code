use std::path::PathBuf;

use gorsee_code_context::build_repo_map;
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

pub struct RepoMapTool {
    root: PathBuf,
}

impl RepoMapTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for RepoMapTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "repo_map".into(),
            description: "Build a compact repository map".into(),
            risk: RiskClass::Read,
            capabilities: vec!["context:repo_map".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let max = args.get("max_files").and_then(Value::as_u64).unwrap_or(300) as usize;
        let map = build_repo_map(&self.root, max);
        let text = serde_json::to_string_pretty(&map)
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        Ok(ToolOutput {
            text,
            json: Some(json!(map)),
            truncated: false,
        })
    }
}
