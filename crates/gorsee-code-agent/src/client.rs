use gorsee_code_neurogate::{ChatRequest, ChatResponse, NeuroGateClient};

use crate::AgentRunError;

pub trait ChatClient {
    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, AgentRunError>;
}

impl ChatClient for NeuroGateClient {
    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, AgentRunError> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| AgentRunError::Runtime(error.to_string()))?;
        runtime
            .block_on(self.chat_completion(request))
            .map_err(AgentRunError::from)
    }
}
