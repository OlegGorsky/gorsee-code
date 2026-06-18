use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::Value;

pub struct FinalAnswerTool;

impl Tool for FinalAnswerTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "final_answer".into(),
            description: "Prepare a concise final answer for the user".into(),
            risk: RiskClass::Read,
            capabilities: vec!["mission:final_answer".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let text = args
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolRuntimeError::Handler("missing text".into()))?;
        Ok(ToolOutput::text(text))
    }
}
