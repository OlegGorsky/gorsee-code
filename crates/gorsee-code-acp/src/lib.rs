use std::{
    fs,
    path::{Path, PathBuf},
};

mod notifications;
pub mod stdio;
mod stdio_support;

use agent_client_protocol::schema::ProtocolVersion;
use gorsee_code_agent::{AgentRunError, ChatClient, EventObserver, TaskRunSummary, TaskRunner};
use gorsee_code_coding_core::{
    LcpTurnResponse, LocalCodingProtocol, OrchestrationPlan, TurnRequest, WorkspaceRef,
};
use gorsee_code_core::AgentProfile;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpTurnInput {
    pub session_id: Option<String>,
    pub workspace_root: String,
    pub prompt: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionRef {
    pub session_id: String,
    pub workspace_root: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpTurnOutput {
    pub protocol_version: ProtocolVersion,
    pub orchestration: OrchestrationPlan,
    pub response: LcpTurnResponse,
    pub summary: TaskRunSummary,
}

#[derive(Debug, Error)]
pub enum AcpAdapterError {
    #[error("workspace does not exist: {0}")]
    InvalidWorkspace(String),
    #[error("acp storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("acp state failed: {0}")]
    State(String),
    #[error("agent run failed: {0}")]
    Agent(#[from] AgentRunError),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AcpAdapter {
    lcp: LocalCodingProtocol,
}

impl AcpAdapter {
    pub fn plan_prompt(
        &self,
        input: AcpTurnInput,
        profiles: Vec<AgentProfile>,
    ) -> OrchestrationPlan {
        self.lcp
            .plan_turn(turn_request_from_acp(input), profiles)
            .orchestration
    }

    pub fn run_prompt<C: ChatClient>(
        &self,
        input: AcpTurnInput,
        profiles: Vec<AgentProfile>,
        client: &C,
    ) -> Result<AcpTurnOutput, AcpAdapterError> {
        let request = turn_request_from_acp(input);
        let workspace = PathBuf::from(&request.workspace.root);
        if !workspace.is_dir() {
            return Err(AcpAdapterError::InvalidWorkspace(
                workspace.display().to_string(),
            ));
        }
        prepare_workspace(&workspace)?;
        let output = TaskRunner::new(workspace.join(".gorsee-code"))
            .run_lcp_turn(request, client, profiles)?;
        Ok(AcpTurnOutput {
            protocol_version: Self::protocol_version(),
            orchestration: output.orchestration,
            response: output.response,
            summary: output.summary,
        })
    }

    pub fn run_prompt_with_event_observer<C: ChatClient>(
        &self,
        input: AcpTurnInput,
        profiles: Vec<AgentProfile>,
        client: &C,
        observer: Box<EventObserver>,
    ) -> Result<AcpTurnOutput, AcpAdapterError> {
        let request = turn_request_from_acp(input);
        let workspace = PathBuf::from(&request.workspace.root);
        if !workspace.is_dir() {
            return Err(AcpAdapterError::InvalidWorkspace(
                workspace.display().to_string(),
            ));
        }
        prepare_workspace(&workspace)?;
        let output = TaskRunner::new(workspace.join(".gorsee-code"))
            .run_lcp_turn_observed(request, client, profiles, observer)?;
        Ok(AcpTurnOutput {
            protocol_version: Self::protocol_version(),
            orchestration: output.orchestration,
            response: output.response,
            summary: output.summary,
        })
    }

    pub fn protocol_version() -> ProtocolVersion {
        ProtocolVersion::V1
    }
}

fn prepare_workspace(workspace: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(workspace.join(".gorsee-code").join("sessions"))?;
    fs::create_dir_all(workspace.join(".gorsee-code").join("artifacts"))?;
    Ok(())
}

pub fn turn_request_from_acp(input: AcpTurnInput) -> TurnRequest {
    TurnRequest {
        workspace: WorkspaceRef {
            root: input.workspace_root,
            branch: None,
            session_id: input.session_id,
        },
        message: input.prompt,
        user_id: input.user_id,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs};

    use gorsee_code_coding_core::{CodingIntent, TranscriptEventKind};
    use gorsee_code_core::{default_agent_matrix, AgentRole};
    use gorsee_code_neurogate::{ChatRequest, ChatResponse};
    use serde_json::json;

    use super::*;

    #[test]
    fn acp_prompt_maps_to_same_orchestrator_plan() {
        let plan = AcpAdapter::default().plan_prompt(
            AcpTurnInput {
                session_id: Some("s1".into()),
                workspace_root: "/repo".into(),
                prompt: "создай модуль".into(),
                user_id: Some("u".into()),
            },
            default_agent_matrix(),
        );

        assert_eq!(plan.request.workspace.session_id.as_deref(), Some("s1"));
        assert_eq!(plan.intent.intent, CodingIntent::Edit);
        assert!(plan.plan.is_some());
    }

    #[test]
    fn adapter_is_bound_to_acp_protocol_version() {
        assert_eq!(AcpAdapter::protocol_version(), ProtocolVersion::V1);
    }

    #[test]
    fn acp_prompt_executes_through_shared_task_runner() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"sample\"\n",
        )
        .unwrap();
        let profiles = default_agent_matrix()
            .into_iter()
            .filter(|profile| profile.role == AgentRole::Architect)
            .collect::<Vec<_>>();
        let client = MockClient::new(
            json!({
                "message": "Привет! Чем помочь?",
                "final_answer": "Привет! Чем помочь?"
            })
            .to_string(),
        );

        let output = AcpAdapter::default()
            .run_prompt(
                AcpTurnInput {
                    session_id: None,
                    workspace_root: temp.path().display().to_string(),
                    prompt: "привет".into(),
                    user_id: Some("acp-user".into()),
                },
                profiles,
                &client,
            )
            .unwrap();

        assert_eq!(output.protocol_version, ProtocolVersion::V1);
        assert_eq!(output.orchestration.intent.intent, CodingIntent::Chat);
        assert_eq!(output.response.session_id, output.summary.session_id);
        assert_eq!(output.response.intent, CodingIntent::Chat);
        assert_eq!(output.response.status, "ready");
        assert!(output.response.diff.is_none());
        assert!(output.response.verification.is_none());
        assert_eq!(
            output
                .response
                .transcript
                .iter()
                .map(|event| &event.kind)
                .collect::<Vec<_>>(),
            vec![
                &TranscriptEventKind::UserMessage,
                &TranscriptEventKind::AssistantMessage
            ]
        );
        assert_eq!(output.summary.agents, vec!["architect"]);
        assert!(temp
            .path()
            .join(".gorsee-code/sessions")
            .join(&output.summary.session_id)
            .join("events.jsonl")
            .exists());
    }

    struct MockClient {
        response: String,
        requests: RefCell<Vec<ChatRequest>>,
    }

    impl MockClient {
        fn new(response: String) -> Self {
            Self {
                response,
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    impl ChatClient for MockClient {
        fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, AgentRunError> {
            self.requests.borrow_mut().push(request.clone());
            Ok(ChatResponse {
                id: Some("mock".into()),
                object: Some("chat.completion".into()),
                choices: Some(vec![
                    json!({ "message": { "content": self.response.clone() } }),
                ]),
                usage: None,
            })
        }
    }
}
