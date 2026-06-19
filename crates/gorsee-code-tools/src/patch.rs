use std::fs;

use gorsee_code_safety::{PathPolicy, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

pub struct ProposePatchTool {
    paths: PathPolicy,
}

pub struct ApplyPatchTool {
    paths: PathPolicy,
}

impl ProposePatchTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl ApplyPatchTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl Tool for ProposePatchTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "propose_patch",
            "Validate and describe a proposed file patch",
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let path = arg(&args, "path")?;
        self.paths.resolve_for_write(path).map_err(handler)?;
        let content = arg(&args, "content")?;
        let summary = format!("Would write {} bytes to {}", content.len(), path);
        Ok(ToolOutput {
            text: summary,
            json: Some(json!({ "path": path, "bytes": content.len(), "dry_run": true })),
            truncated: false,
        })
    }
}

impl Tool for ApplyPatchTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "apply_patch",
            "Write approved file content inside the workspace",
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let path = arg(&args, "path")?;
        let content = arg(&args, "content")?;
        let target = self.paths.resolve_for_write(path).map_err(handler)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(handler)?;
        }
        fs::write(&target, content).map_err(handler)?;
        Ok(ToolOutput::text(format!(
            "wrote {} bytes to {}",
            content.len(),
            path
        )))
    }
}

fn arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolRuntimeError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| handler(format!("missing {name}")))
}

fn manifest(name: &str, description: &str) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        description: description.into(),
        risk: RiskClass::Write,
        capabilities: vec!["files:write".into()],
    }
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}
