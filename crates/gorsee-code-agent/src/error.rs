use gorsee_code_artifacts::ArtifactError;
use gorsee_code_neurogate::NeuroGateError;
use gorsee_code_safety::PathPolicyError;
use gorsee_code_session::SessionStoreError;
use gorsee_code_tool_runtime::ToolRuntimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentRunError {
    #[error("session failed: {0}")]
    Session(#[from] SessionStoreError),
    #[error("artifact failed: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("neurogate failed: {0}")]
    NeuroGate(#[from] NeuroGateError),
    #[error("tool failed: {0}")]
    Tool(#[from] ToolRuntimeError),
    #[error("path policy failed: {0}")]
    Path(#[from] PathPolicyError),
    #[error("waiting for approval: {0}")]
    WaitingApproval(String),
    #[error("runtime failed: {0}")]
    Runtime(String),
    #[error("invalid model response: {0}")]
    InvalidModelResponse(String),
}
