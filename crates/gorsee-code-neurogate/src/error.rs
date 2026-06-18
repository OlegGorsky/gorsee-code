use thiserror::Error;

#[derive(Debug, Error)]
pub enum NeuroGateError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Unexpected(String),
}
