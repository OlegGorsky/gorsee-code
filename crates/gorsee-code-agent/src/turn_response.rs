use std::{fs, path::Path};

use gorsee_code_coding_core::{
    LcpApprovalSummary, LcpTurnResponse, LcpTurnResponseInput, LcpUsageSnapshot,
    LocalCodingProtocol, OrchestrationPlan,
};
use gorsee_code_core::AgentProfile;
use gorsee_code_session::{ApprovalRecord, SessionStore};
use gorsee_code_usage::TokenLedger;

use crate::{summary::TaskRunSummary, AgentRunError};

#[derive(Debug, Clone, PartialEq)]
pub struct TaskTurnOutput {
    pub orchestration: OrchestrationPlan,
    pub response: LcpTurnResponse,
    pub summary: TaskRunSummary,
}

pub fn build_lcp_response(
    workspace: &Path,
    orchestration: OrchestrationPlan,
    session_id: &str,
) -> Result<LcpTurnResponse, AgentRunError> {
    let store = SessionStore::new(
        workspace.join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    build_lcp_response_from_store(&store, orchestration, session_id)
}

pub(crate) fn build_lcp_response_from_store(
    store: &SessionStore,
    orchestration: OrchestrationPlan,
    session_id: &str,
) -> Result<LcpTurnResponse, AgentRunError> {
    let manifest = store.read_manifest(session_id)?;
    let events = store.read_events(session_id)?;
    let approvals = store.read_approvals(session_id)?;
    let cached_tokens = cached_tokens_from_ledger(store, session_id);
    Ok(LocalCodingProtocol::default().turn_response(
        orchestration,
        LcpTurnResponseInput {
            session_id: &manifest.id,
            turn_id: None,
            status: client_session_status(&manifest.status),
            events: &events,
            usage: LcpUsageSnapshot::new(manifest.budget.tokens_used, manifest.budget.tokens_limit)
                .with_cached_tokens(cached_tokens),
            approvals: approvals.iter().map(approval_summary).collect(),
        },
    ))
}

fn cached_tokens_from_ledger(store: &SessionStore, session_id: &str) -> u64 {
    fs::read_to_string(store.session_dir(session_id).join("token-ledger.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<TokenLedger>(&text).ok())
        .map(|ledger| ledger.totals().cached_tokens)
        .unwrap_or(0)
}

pub(crate) fn waiting_summary(
    store: &SessionStore,
    session_id: String,
    agents: Vec<AgentProfile>,
) -> Result<TaskRunSummary, AgentRunError> {
    let events = store.read_events(&session_id)?;
    Ok(TaskRunSummary {
        session_id,
        events: events.len(),
        agents: agents
            .into_iter()
            .map(|agent| agent.id().to_string())
            .collect(),
        artifacts: Vec::new(),
    })
}

fn client_session_status(status: &str) -> &str {
    match status {
        "finished" => "ready",
        other => other,
    }
}

fn approval_summary(approval: &ApprovalRecord) -> LcpApprovalSummary {
    LcpApprovalSummary {
        id: approval.id.clone(),
        agent_id: approval.agent_id.clone(),
        tool_name: approval.tool_name.clone(),
        risk: format!("{:?}", approval.risk).to_lowercase(),
        status: format!("{:?}", approval.status).to_lowercase(),
    }
}
