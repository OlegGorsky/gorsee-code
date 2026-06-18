use gorsee_code_core::ModelCapability;
use gorsee_code_gateway::HealthResponse;
use gorsee_code_ui_state::SessionView;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayClientError {
    #[error("gateway request failed: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct GatewayClient {
    base_url: String,
    http: reqwest::Client,
}

impl GatewayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn health(&self) -> Result<HealthResponse, GatewayClientError> {
        Ok(self.get("/health").await?)
    }

    pub async fn sessions(&self) -> Result<Vec<SessionView>, GatewayClientError> {
        Ok(self
            .get::<Envelope<SessionView>>("/v1/sessions")
            .await?
            .data)
    }

    pub async fn capabilities(&self) -> Result<Vec<ModelCapability>, GatewayClientError> {
        Ok(self
            .get::<Envelope<ModelCapability>>("/v1/capabilities")
            .await?
            .data)
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, reqwest::Error> {
        self.http
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
    }
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: Vec<T>,
}
