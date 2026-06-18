use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: Option<String>,
    pub object: Option<String>,
    pub choices: Option<Vec<Value>>,
    pub usage: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatStreamChunk {
    pub data: Value,
}

pub fn parse_stream_line(line: &str) -> Result<Option<ChatStreamChunk>, serde_json::Error> {
    let Some(payload) = line.trim().strip_prefix("data:") else {
        return Ok(None);
    };
    let payload = payload.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(None);
    }
    Ok(Some(ChatStreamChunk {
        data: serde_json::from_str(payload)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_compatible_stream_line() {
        let chunk = parse_stream_line(r#"data: {"choices":[{"delta":{"content":"hi"}}]}"#)
            .unwrap()
            .unwrap();

        assert_eq!(chunk.data["choices"][0]["delta"]["content"], "hi");
        assert_eq!(parse_stream_line("data: [DONE]").unwrap(), None);
    }
}
