use thiserror::Error;

#[derive(Debug, Error)]
pub enum NeuroGateError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("http status {status} for url ({url}): {body}")]
    Status {
        status: u16,
        url: String,
        body: String,
    },
    #[error("unexpected response: {0}")]
    Unexpected(String),
}
