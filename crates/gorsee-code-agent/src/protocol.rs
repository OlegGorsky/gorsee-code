use gorsee_code_neurogate::ChatResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AgentRunError;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct ModelTurn {
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<ModelToolCall>,
    #[serde(default)]
    pub(crate) final_answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct ModelToolCall {
    pub(crate) name: String,
    #[serde(default = "empty_args")]
    pub(crate) args: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AgentAnswer {
    pub(crate) agent_id: String,
    pub(crate) answer: String,
}

impl AgentAnswer {
    pub(crate) fn new(agent_id: impl Into<String>, answer: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            answer: answer.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ToolResult {
    pub(crate) agent_id: String,
    pub(crate) name: String,
    pub(crate) ok: bool,
    pub(crate) text: String,
}

impl ToolResult {
    pub(crate) fn success(
        agent_id: impl Into<String>,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            ok: true,
            text: text.into(),
        }
    }

    pub(crate) fn failure(
        agent_id: impl Into<String>,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            ok: false,
            text: text.into(),
        }
    }
}

pub(crate) fn parse_response(response: &ChatResponse) -> Result<ModelTurn, AgentRunError> {
    let content = extract_content(response)
        .ok_or_else(|| AgentRunError::InvalidModelResponse("missing content".into()))?;
    parse_turn(&content)
}

fn parse_turn(content: &str) -> Result<ModelTurn, AgentRunError> {
    let json = json_payload(content).ok_or_else(|| {
        AgentRunError::InvalidModelResponse("response does not contain json".into())
    })?;
    serde_json::from_str(json)
        .map_err(|error| AgentRunError::InvalidModelResponse(format!("{error}: {json}")))
}

fn extract_content(response: &ChatResponse) -> Option<String> {
    response
        .choices
        .as_ref()?
        .iter()
        .find_map(extract_choice_content)
}

fn extract_choice_content(choice: &Value) -> Option<String> {
    if let Some(content) = choice.as_str() {
        return Some(content.to_string());
    }
    let content = choice
        .pointer("/message/content")
        .or_else(|| choice.pointer("/delta/content"))
        .or_else(|| choice.get("text"))?;
    content.as_str().map(str::to_string)
}

fn json_payload(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        return fenced_json(trimmed);
    }
    object_slice(trimmed)
}

fn fenced_json(content: &str) -> Option<&str> {
    let body = content.split_once('\n')?.1;
    let body = body.strip_suffix("```").unwrap_or(body).trim();
    object_slice(body)
}

fn object_slice(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    content.get(start..=end)
}

fn empty_args() -> Value {
    Value::Object(Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_openai_message_content() {
        let response = ChatResponse {
            id: None,
            object: None,
            choices: Some(vec![json!({
                "message": { "content": r#"{"final_answer":"done"}"# }
            })]),
            usage: None,
        };

        let turn = parse_response(&response).unwrap();

        assert_eq!(turn.final_answer, Some("done".into()));
    }

    #[test]
    fn parses_fenced_json_content() {
        let turn = parse_turn("```json\n{\"message\":\"ok\"}\n```").unwrap();

        assert_eq!(turn.message, Some("ok".into()));
    }
}
