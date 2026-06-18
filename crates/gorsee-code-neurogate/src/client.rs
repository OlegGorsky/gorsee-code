use futures_util::StreamExt;
use gorsee_code_core::ModelCapability;
use gorsee_code_limits::{parse_usage_windows, UsageWindow};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;

use crate::{
    chat::{parse_stream_line, ChatRequest, ChatResponse, ChatStreamChunk},
    error::NeuroGateError,
    models::{parse_models_response, ModelListResponse},
};

#[derive(Clone)]
pub struct NeuroGateClient {
    base_url: String,
    http: reqwest::Client,
}

impl NeuroGateClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl AsRef<str>,
    ) -> Result<Self, NeuroGateError> {
        let mut headers = HeaderMap::new();
        let value = format!("Bearer {}", api_key.as_ref());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&value)
                .map_err(|error| NeuroGateError::Unexpected(error.to_string()))?,
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    pub async fn list_models(&self) -> Result<Vec<ModelCapability>, NeuroGateError> {
        let response = self
            .http
            .get(format!("{}/models", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ModelListResponse>()
            .await?;
        Ok(parse_models_response(response))
    }

    pub async fn account_limits(&self) -> Result<Vec<UsageWindow>, NeuroGateError> {
        let response = self
            .http
            .get(format!("{}/me", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        Ok(parse_usage_windows(&response))
    }

    pub async fn chat_completion(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, NeuroGateError> {
        let response = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;
        Ok(response)
    }

    pub async fn chat_completion_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Vec<ChatStreamChunk>, NeuroGateError> {
        let mut request = request.clone();
        request.stream = true;
        let mut stream = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .bytes_stream();
        let mut buffer = String::new();
        let mut chunks = Vec::new();
        while let Some(bytes) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&bytes?));
            drain_stream_lines(&mut buffer, &mut chunks, false)?;
        }
        drain_stream_lines(&mut buffer, &mut chunks, true)?;
        Ok(chunks)
    }
}

fn drain_stream_lines(
    buffer: &mut String,
    chunks: &mut Vec<ChatStreamChunk>,
    final_chunk: bool,
) -> Result<(), NeuroGateError> {
    while let Some(index) = buffer.find('\n') {
        let line = buffer.drain(..=index).collect::<String>();
        push_stream_line(&line, chunks)?;
    }
    if final_chunk && !buffer.trim().is_empty() {
        let line = std::mem::take(buffer);
        push_stream_line(&line, chunks)?;
    }
    Ok(())
}

fn push_stream_line(line: &str, chunks: &mut Vec<ChatStreamChunk>) -> Result<(), NeuroGateError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let parsed =
        parse_stream_line(line).map_err(|error| NeuroGateError::Unexpected(error.to_string()))?;
    if let Some(chunk) = parsed {
        chunks.push(chunk);
    }
    Ok(())
}
