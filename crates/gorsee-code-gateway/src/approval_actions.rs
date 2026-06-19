use std::{fs, path::Path};

use axum::http::StatusCode;
use gorsee_code_agent::{AgentRunError, TaskRunSummary, TaskRunner};
use gorsee_code_config::{default_config, GorseeConfig};
use gorsee_code_neurogate::NeuroGateClient;
use gorsee_code_session::{ApprovalDecision, ApprovalRecord};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::GatewayState;

pub(crate) enum ApprovalAction {
    Recorded(ApprovalRecord),
    Executed {
        approval: ApprovalRecord,
        summary: TaskRunSummary,
    },
    WaitingApproval {
        approval: ApprovalRecord,
        next_approval_id: String,
    },
}

impl ApprovalAction {
    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Recorded(approval) => with_mode(json!(approval), "record_only"),
            Self::Executed { approval, summary } => {
                with_extra(json!(approval), "executed", json!({ "summary": summary }))
            }
            Self::WaitingApproval {
                approval,
                next_approval_id,
            } => with_extra(
                json!(approval),
                "waiting_approval",
                json!({ "next_approval_id": next_approval_id }),
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ApprovalActionError {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

pub(crate) async fn decide(
    state: GatewayState,
    session_id: String,
    approval_id: String,
    decision: ApprovalDecision,
) -> Result<ApprovalAction, ApprovalActionError> {
    let has_execution = state
        .has_pending_execution(&session_id)
        .map_err(session_error)?
        .ok_or_else(not_found)?;
    if !has_execution {
        return state
            .decide_approval(&session_id, &approval_id, decision)
            .map_err(session_error)?
            .map(ApprovalAction::Recorded)
            .ok_or_else(not_found);
    }

    let client = live_client(state.workspace_path())?.ok_or_else(missing_auth)?;
    resume_saved_execution(state, session_id, approval_id, decision, client).await
}

async fn resume_saved_execution(
    state: GatewayState,
    session_id: String,
    approval_id: String,
    decision: ApprovalDecision,
    client: NeuroGateClient,
) -> Result<ApprovalAction, ApprovalActionError> {
    let session_root = state.session_root();
    let run_session_id = session_id.clone();
    let run_approval_id = approval_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let runner = TaskRunner::new(session_root);
        runner.resume_after_decision(&run_session_id, &run_approval_id, decision, &client)
    })
    .await
    .map_err(join_error)?;

    match result {
        Ok(summary) => Ok(ApprovalAction::Executed {
            approval: decided_approval(&state, &session_id, &approval_id)?,
            summary,
        }),
        Err(AgentRunError::WaitingApproval(next_approval_id)) => {
            Ok(ApprovalAction::WaitingApproval {
                approval: decided_approval(&state, &session_id, &approval_id)?,
                next_approval_id,
            })
        }
        Err(error) => Err(agent_error(error)),
    }
}

fn live_client(root: &Path) -> Result<Option<NeuroGateClient>, ApprovalActionError> {
    let Some(api_key) = api_key(root)? else {
        return Ok(None);
    };
    let config = GorseeConfig::load(root.join("gorsee-code.toml"))
        .unwrap_or_else(|_| default_config(project_name(root)));
    NeuroGateClient::new(config.neurogate.endpoint, api_key)
        .map(Some)
        .map_err(|error| ApprovalActionError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "client_error",
            message: error.to_string(),
        })
}

fn api_key(root: &Path) -> Result<Option<String>, ApprovalActionError> {
    for name in ["NEUROGATE_API_KEY", "GORSEE_NEUROGATE_API_KEY"] {
        if let Some(value) = non_empty(std::env::var(name).ok()) {
            return Ok(Some(value));
        }
    }
    read_local_key(root)
}

fn read_local_key(root: &Path) -> Result<Option<String>, ApprovalActionError> {
    let path = root.join(".gorsee-code").join("auth.json");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(auth_error(error)),
    };
    let auth: AuthFile = serde_json::from_str(&text).map_err(auth_error)?;
    Ok(non_empty(Some(auth.api_key)))
}

fn decided_approval(
    state: &GatewayState,
    session_id: &str,
    approval_id: &str,
) -> Result<ApprovalRecord, ApprovalActionError> {
    state
        .approval(session_id, approval_id)
        .map_err(session_error)?
        .ok_or_else(|| session_error(format!("approval not found: {approval_id}")))
}

fn with_extra(mut value: Value, mode: &str, extra: Value) -> Value {
    value = with_mode(value, mode);
    if let (Value::Object(map), Value::Object(extra)) = (&mut value, extra) {
        map.extend(extra);
    }
    value
}

fn with_mode(mut value: Value, mode: &str) -> Value {
    if let Value::Object(map) = &mut value {
        map.insert("mode".into(), json!(mode));
    }
    value
}

fn project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("gorsee-code")
        .to_string()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn missing_auth() -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::CONFLICT,
        code: "missing_auth",
        message: "NeuroGate API key is required to resume saved execution".into(),
    }
}

fn not_found() -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::NOT_FOUND,
        code: "session_not_found",
        message: "session not found".into(),
    }
}

fn auth_error(error: impl std::fmt::Display) -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "auth_error",
        message: error.to_string(),
    }
}

fn session_error(error: impl std::fmt::Display) -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "session_error",
        message: error.to_string(),
    }
}

fn agent_error(error: AgentRunError) -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "resume_failed",
        message: error.to_string(),
    }
}

fn join_error(error: tokio::task::JoinError) -> ApprovalActionError {
    ApprovalActionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        code: "resume_failed",
        message: error.to_string(),
    }
}

#[derive(Deserialize)]
struct AuthFile {
    api_key: String,
}
