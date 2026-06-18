use std::collections::BTreeMap;

use gorsee_code_safety::{Decision, OutputBounds, PermissionPolicy, Redactor};
use serde_json::Value;
use thiserror::Error;

use crate::{ToolManifest, ToolOutput};

pub trait Tool: Send + Sync {
    fn manifest(&self) -> ToolManifest;
    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError>;
}

#[derive(Debug, Error)]
pub enum ToolRuntimeError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("tool requires approval: {0}")]
    RequiresApproval(String),
    #[error("tool denied by policy: {0}")]
    PermissionDenied(String),
    #[error("tool failed: {0}")]
    Handler(String),
}

pub struct ToolRegistry {
    tools: BTreeMap<String, Box<dyn Tool>>,
    policy: PermissionPolicy,
    bounds: OutputBounds,
    redactor: Redactor,
}

impl ToolRegistry {
    pub fn new(policy: PermissionPolicy, bounds: OutputBounds, redactor: Redactor) -> Self {
        Self {
            tools: BTreeMap::new(),
            policy,
            bounds,
            redactor,
        }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.manifest().name, Box::new(tool));
    }

    pub fn manifests(&self) -> Vec<ToolManifest> {
        self.tools.values().map(|tool| tool.manifest()).collect()
    }

    pub fn run(&self, name: &str, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolRuntimeError::UnknownTool(name.into()))?;
        let manifest = tool.manifest();
        match self.policy.decide(manifest.risk) {
            Decision::Allow => self.run_allowed(tool.as_ref(), args),
            Decision::Ask => Err(ToolRuntimeError::RequiresApproval(name.into())),
            Decision::Deny => Err(ToolRuntimeError::PermissionDenied(name.into())),
        }
    }

    fn run_allowed(&self, tool: &dyn Tool, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let mut output = tool.run(args)?;
        let bounded = self.bounds.apply(&output.text);
        output.text = self.redactor.redact(&bounded.text);
        output.truncated |= bounded.truncated;
        Ok(output)
    }
}
