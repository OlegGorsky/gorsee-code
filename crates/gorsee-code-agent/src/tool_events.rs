use gorsee_code_core::{AgentProfile, EventKind};
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{ToolOutput, ToolRuntimeError};
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
    output: ToolOutput,
) -> Result<ToolResult, AgentRunError> {
    let ok = output_status_ok(&output).unwrap_or(true);
    let payload = tool_finished_payload(call, &output);
    sink.push(Some(agent.id()), EventKind::ToolFinished, payload)?;
    Ok(ToolResult::output(
        agent.id(),
        call.name.clone(),
        ok,
        output,
    ))
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

fn tool_finished_payload(call: &ModelToolCall, output: &ToolOutput) -> Value {
    let mut payload = json!({
        "name": call.name,
        "output": output.text,
        "truncated": output.truncated,
    });
    if let Some(json_value) = &output.json {
        if let Some(status) = json_value.get("status").and_then(Value::as_str) {
            payload["status"] = Value::String(status.to_string());
        }
        if let Some(exit_status) = json_value.get("exit_status") {
            payload["exit_status"] = exit_status.clone();
        }
        payload["json"] = json_value.clone();
    }
    payload
}

fn output_status_ok(output: &ToolOutput) -> Option<bool> {
    let status = output.json.as_ref()?.get("status")?.as_str()?;
    Some(!matches!(status, "failed" | "error"))
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

#[cfg(test)]
mod tests {
    use gorsee_code_core::{AgentProfile, AgentRole, EventKind};
    use gorsee_code_safety::Redactor;
    use gorsee_code_session::{SessionManifest, SessionStore};
    use gorsee_code_tool_runtime::ToolOutput;
    use serde_json::json;

    use crate::{events::EventSink, protocol::ModelToolCall};

    use super::*;

    #[test]
    fn tool_success_preserves_structured_json_result() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(temp.path(), Redactor::default());
        store
            .create(&SessionManifest::new("s1", "/repo", "main"))
            .unwrap();
        let mut sink = EventSink::new(&store, "s1".into());
        let call = ModelToolCall {
            name: "git_diff".into(),
            args: json!({}),
        };
        let output = ToolOutput {
            text: "files_changed=1".into(),
            json: Some(json!({ "diff": { "summary": { "files_changed": 1 } } })),
            truncated: false,
        };

        let result = record_tool_success(&mut sink, &agent(), &call, output).unwrap();

        assert_eq!(
            result.json.as_ref().unwrap()["diff"]["summary"]["files_changed"],
            1
        );
        assert!(result.ok);
        let events = store.read_events("s1").unwrap();
        let event = events
            .iter()
            .find(|event| event.kind == EventKind::ToolFinished)
            .unwrap();
        assert_eq!(event.payload["json"]["diff"]["summary"]["files_changed"], 1);
    }

    #[test]
    fn failed_structured_tool_status_marks_result_not_ok() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(temp.path(), Redactor::default());
        store
            .create(&SessionManifest::new("s1", "/repo", "main"))
            .unwrap();
        let mut sink = EventSink::new(&store, "s1".into());
        let call = ModelToolCall {
            name: "run_test".into(),
            args: json!({}),
        };
        let output = ToolOutput {
            text: "tests failed".into(),
            json: Some(json!({ "status": "failed", "exit_status": 101 })),
            truncated: false,
        };

        let result = record_tool_success(&mut sink, &agent(), &call, output).unwrap();

        assert!(!result.ok);
        let event = store.read_events("s1").unwrap().pop().unwrap();
        assert_eq!(event.payload["status"], "failed");
        assert_eq!(event.payload["exit_status"], 101);
    }

    fn agent() -> AgentProfile {
        AgentProfile {
            role: AgentRole::Validator,
            model: "test".into(),
            reasoning: "low".into(),
            tools: vec!["diff".into()],
            budget_tokens: 1000,
            temperature: 0.0,
        }
    }
}
