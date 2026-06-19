use gorsee_code_core::{AgentProfile, EventKind};
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::ToolRuntimeError;
use serde_json::{json, Value};

use crate::{
    events::EventSink,
    protocol::{ModelToolCall, ToolResult},
    AgentRunError,
};

pub(crate) fn record_approval_event(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: &str,
    kind: EventKind,
) -> Result<(), AgentRunError> {
    sink.push(
        Some(agent.id()),
        kind,
        json!({ "approval_id": approval_id, "tool": call.name }),
    )?;
    Ok(())
}

pub(crate) fn record_tool_request(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: Option<&str>,
    risk: Option<RiskClass>,
) -> Result<(), AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::ToolRequested,
        json!({ "name": call.name, "args": redact_args(&call.args), "approval_id": approval_id, "risk": risk }),
    )?;
    Ok(())
}

pub(crate) fn record_patch_proposal(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<(), AgentRunError> {
    if call.name == "propose_patch" || call.name == "apply_patch" {
        sink.push(
            Some(agent.id()),
            EventKind::PatchProposed,
            json!({ "name": call.name, "args": redact_args(&call.args), "approval_id": approval_id }),
        )?;
    }
    Ok(())
}

pub(crate) fn record_tool_success(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    text: String,
) -> Result<ToolResult, AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::ToolFinished,
        json!({ "name": call.name, "output": text }),
    )?;
    Ok(ToolResult::success(agent.id(), call.name.clone(), text))
}

pub(crate) fn record_patch_applied(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<(), AgentRunError> {
    if call.name == "apply_patch" {
        sink.push(
            Some(agent.id()),
            EventKind::PatchApplied,
            json!({ "approval_id": approval_id, "args": redact_args(&call.args) }),
        )?;
    }
    Ok(())
}

pub(crate) fn record_tool_failure(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    error: ToolRuntimeError,
) -> Result<ToolResult, AgentRunError> {
    let text = error.to_string();
    sink.push(
        Some(agent.id()),
        EventKind::Error,
        json!({ "name": call.name, "error": text }),
    )?;
    Ok(ToolResult::failure(agent.id(), call.name.clone(), text))
}

pub(crate) fn record_message(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    message: Option<&str>,
) -> Result<(), AgentRunError> {
    if let Some(message) = message.filter(|message| !message.trim().is_empty()) {
        sink.push(
            Some(agent.id()),
            EventKind::AgentMessage,
            json!({ "message": message }),
        )?;
    }
    Ok(())
}

pub(crate) fn redact_args(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(redact_args).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_secret_key(key) {
                        (key.clone(), Value::String("[REDACTED]".into()))
                    } else {
                        (key.clone(), redact_args(value))
                    }
                })
                .collect(),
        ),
        Value::String(text) if text.to_ascii_lowercase().contains("bearer ") => {
            Value::String("[REDACTED]".into())
        }
        _ => value.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "authorization",
        "api_key",
        "apikey",
        "cookie",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}
