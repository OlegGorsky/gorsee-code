use gorsee_code_neurogate::ChatResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AgentRunError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ModelTurn {
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<ModelToolCall>,
    #[serde(default)]
    pub(crate) final_answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ModelToolCall {
    pub(crate) name: String,
    #[serde(default = "empty_args")]
    pub(crate) args: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    let payload = json_payload(content).ok_or_else(|| {
        AgentRunError::InvalidModelResponse("response does not contain json".into())
    })?;
    parse_payload(payload)
        .map_err(|error| AgentRunError::InvalidModelResponse(format!("invalid json: {error}")))
}

fn parse_payload(payload: &str) -> Result<ModelTurn, serde_json::Error> {
    let mut turn = ModelTurn {
        message: None,
        tool_calls: Vec::new(),
        final_answer: None,
    };
    for value in json_values(payload)? {
        merge_turn(&mut turn, serde_json::from_value(value)?);
    }
    Ok(turn)
}

fn merge_turn(target: &mut ModelTurn, turn: ModelTurn) {
    if turn
        .message
        .as_deref()
        .map(|message| !message.trim().is_empty())
        .unwrap_or(false)
    {
        target.message = turn.message;
    }
    target.tool_calls.extend(turn.tool_calls);
    if turn
        .final_answer
        .as_deref()
        .map(|answer| !answer.trim().is_empty())
        .unwrap_or(false)
    {
        target.final_answer = turn.final_answer;
    }
}

fn json_values(payload: &str) -> Result<Vec<Value>, serde_json::Error> {
    let mut values = Vec::new();
    let mut rest = payload.trim();
    while let Some(start) = rest.find('{') {
        let candidate = &rest[start..];
        let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
        if let Some(value) = stream.next() {
            values.push(value?);
            rest = &candidate[stream.byte_offset()..];
        } else {
            break;
        }
    }
    Ok(values)
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
    let payload = if trimmed.starts_with("```") {
        fenced_json(trimmed)?
    } else {
        trimmed
    };
    payload.contains('{').then_some(payload)
}

fn fenced_json(content: &str) -> Option<&str> {
    let body = content.split_once('\n')?.1;
    Some(body.strip_suffix("```").unwrap_or(body).trim())
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

    #[test]
    fn parses_concatenated_json_turns() {
        let turn = parse_turn(
            r#"{"message":"уточните задачу"}{"final_answer":"Please provide more detail."}"#,
        )
        .unwrap();

        assert_eq!(turn.message, Some("уточните задачу".into()));
        assert_eq!(
            turn.final_answer,
            Some("Please provide more detail.".into())
        );
    }

    #[test]
    fn invalid_json_error_does_not_echo_payload() {
        let error = parse_turn(r#"{"message": "broken""#)
            .unwrap_err()
            .to_string();

        assert!(error.contains("invalid model response"));
        assert!(!error.contains("broken"));
    }
}
