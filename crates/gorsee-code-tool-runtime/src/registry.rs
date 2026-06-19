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

    pub fn manifest(&self, name: &str) -> Result<ToolManifest, ToolRuntimeError> {
        Ok(self.tool(name)?.manifest())
    }

    pub fn approval_required(&self, name: &str) -> Result<Option<ToolManifest>, ToolRuntimeError> {
        let manifest = self.manifest(name)?;
        Ok(match self.policy.decide(manifest.risk) {
            Decision::Ask => Some(manifest),
            Decision::Allow | Decision::Deny => None,
        })
    }

    pub fn run(&self, name: &str, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let tool = self.tool(name)?;
        let manifest = tool.manifest();
        match self.policy.decide(manifest.risk) {
            Decision::Allow => self.run_allowed(tool, args),
            Decision::Ask => Err(ToolRuntimeError::RequiresApproval(name.into())),
            Decision::Deny => Err(ToolRuntimeError::PermissionDenied(name.into())),
        }
    }

    pub fn run_approved(&self, name: &str, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let tool = self.tool(name)?;
        let manifest = tool.manifest();
        match self.policy.decide(manifest.risk) {
            Decision::Allow | Decision::Ask => self.run_allowed(tool, args),
            Decision::Deny => Err(ToolRuntimeError::PermissionDenied(name.into())),
        }
    }

    fn tool(&self, name: &str) -> Result<&dyn Tool, ToolRuntimeError> {
        self.tools
            .get(name)
            .map(|tool| tool.as_ref())
            .ok_or_else(|| ToolRuntimeError::UnknownTool(name.into()))
    }

    fn run_allowed(&self, tool: &dyn Tool, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let mut output = tool.run(args)?;
        let bounded = self.bounds.apply(&output.text);
        output.text = self.redactor.redact(&bounded.text);
        output.truncated |= bounded.truncated;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use gorsee_code_safety::{OutputBounds, PermissionPolicy, Redactor, RiskClass};
    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn write_tool_reports_approval_requirement_before_running() {
        let registry = registry_with(EchoTool);

        let approval = registry.approval_required("echo_write").unwrap();
        let error = registry.run("echo_write", json!({})).unwrap_err();

        assert_eq!(
            approval.map(|manifest| manifest.risk),
            Some(RiskClass::Write)
        );
        assert!(matches!(error, ToolRuntimeError::RequiresApproval(_)));
    }

    #[test]
    fn approved_run_executes_policy_ask_tool() {
        let registry = registry_with(EchoTool);

        let output = registry
            .run_approved("echo_write", json!({ "text": "ok" }))
            .unwrap();

        assert_eq!(output.text, "ok");
    }

    fn registry_with<T: Tool + 'static>(tool: T) -> ToolRegistry {
        let mut registry = ToolRegistry::new(
            PermissionPolicy::balanced(),
            OutputBounds::default(),
            Redactor::default(),
        );
        registry.register(tool);
        registry
    }

    struct EchoTool;

    impl Tool for EchoTool {
        fn manifest(&self) -> ToolManifest {
            ToolManifest {
                name: "echo_write".into(),
                description: "Echo write".into(),
                risk: RiskClass::Write,
                capabilities: vec!["test:echo".into()],
            }
        }

        fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
            Ok(ToolOutput::text(
                args.get("text").and_then(Value::as_str).unwrap_or_default(),
            ))
        }
    }
}
