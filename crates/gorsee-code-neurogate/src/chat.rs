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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<String>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            stream: false,
            prompt_cache_key: None,
            prompt_cache_retention: None,
        }
    }

    pub fn with_prompt_cache_key(mut self, key: impl Into<String>) -> Self {
        self.prompt_cache_key = Some(key.into());
        self
    }

    pub fn with_prompt_cache_retention(mut self, retention: impl Into<String>) -> Self {
        self.prompt_cache_retention = Some(retention.into());
        self
    }
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

    #[test]
    fn omits_prompt_cache_fields_by_default() {
        let request = ChatRequest::new("glm-5.1", vec![ChatMessage::user("hi")]);
        let payload = serde_json::to_value(request).unwrap();

        assert!(payload.get("prompt_cache_key").is_none());
        assert!(payload.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn serializes_prompt_cache_hints_when_enabled() {
        let request = ChatRequest::new("glm-5.1", vec![ChatMessage::user("hi")])
            .with_prompt_cache_key("gorsee:agent:v1:test")
            .with_prompt_cache_retention("24h");
        let payload = serde_json::to_value(request).unwrap();

        assert_eq!(payload["prompt_cache_key"], "gorsee:agent:v1:test");
        assert_eq!(payload["prompt_cache_retention"], "24h");
    }
}
