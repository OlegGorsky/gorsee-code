use std::path::Path;

use anyhow::{anyhow, Result};
use gorsee_code_agent::{AgentRunError, ChatClient, TaskRunSummary, TaskRunner};
use gorsee_code_safety::{Redactor, RiskClass};
use gorsee_code_session::{ApprovalDecision, ApprovalRecord, SessionStore};

use crate::{
    commands_extra::{require_live_client, session_ids_by_started_at},
    paths,
};

pub fn list(root: &Path) -> Result<String> {
    let Some((_, approvals)) = latest_pending(root)? else {
        return Ok("approvals: none\n".into());
    };
    let mut out = "approvals: pending\n".to_string();
    for approval in approvals {
        out.push_str(&format!(
            "- {} agent={} tool={} risk={}\n",
            approval.id,
            approval.agent_id,
            approval.tool_name,
            risk_label(approval.risk)
        ));
    }
    Ok(out)
}

pub fn decide(
    root: &Path,
    approval_id: &str,
    decision: ApprovalDecision,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    let (session_id, _) = find_approval(root, approval_id)?;
    ensure_saved_execution(root, &session_id, approval_id)?;
    let client = require_live_client(root, env_key, global_auth_path)?;
    resume_decision(root, approval_id, decision, &session_id, &client)
}

#[cfg(test)]
fn decide_with_client<C: ChatClient>(
    root: &Path,
    approval_id: &str,
    decision: ApprovalDecision,
    client: &C,
) -> Result<String> {
    let (session_id, _) = find_approval(root, approval_id)?;
    resume_decision(root, approval_id, decision, &session_id, client)
}

fn resume_decision<C: ChatClient>(
    root: &Path,
    approval_id: &str,
    decision: ApprovalDecision,
    session_id: &str,
    client: &C,
) -> Result<String> {
    let runner = TaskRunner::new(paths::local_dir(root));
    match runner.resume_after_decision(session_id, approval_id, decision, client) {
        Ok(summary) => Ok(finished_output(decision, approval_id, &summary)),
        Err(AgentRunError::WaitingApproval(next_approval_id)) => Ok(waiting_output(
            decision,
            approval_id,
            session_id,
            &next_approval_id,
        )),
        Err(error) => Err(error.into()),
    }
}

fn ensure_saved_execution(root: &Path, session_id: &str, approval_id: &str) -> Result<()> {
    let path = paths::sessions_dir(root)
        .join(session_id)
        .join("execution.json");
    if !path.is_file() {
        return Err(anyhow!(
            "approval cannot be resumed: saved execution is missing for {approval_id}"
        ));
    }
    Ok(())
}

fn latest_pending(root: &Path) -> Result<Option<(String, Vec<ApprovalRecord>)>> {
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    for id in sorted_sessions(root)?.into_iter().rev() {
        let pending = store.pending_approvals(&id)?;
        if !pending.is_empty() {
            return Ok(Some((id, pending)));
        }
    }
    Ok(None)
}

fn find_approval(root: &Path, approval_id: &str) -> Result<(String, ApprovalRecord)> {
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    for id in sorted_sessions(root)?.into_iter().rev() {
        if let Some(record) = store
            .read_approvals(&id)?
            .into_iter()
            .find(|approval| approval.id == approval_id)
        {
            return Ok((id, record));
        }
    }
    Err(anyhow!("approval not found: {approval_id}"))
}

fn finished_output(
    decision: ApprovalDecision,
    approval_id: &str,
    summary: &TaskRunSummary,
) -> String {
    format!(
        "{}: {}\nstatus: ready\nturn: finished\nsession: {}\nevents: {}\nagents: {}\nartifacts: {}\n",
        command_label(decision),
        approval_id,
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    )
}

fn waiting_output(
    decision: ApprovalDecision,
    approval_id: &str,
    session_id: &str,
    next_approval_id: &str,
) -> String {
    format!(
        "{}: {}\nstatus: waiting_approval\nsession: {}\napproval: {}\n",
        command_label(decision),
        approval_id,
        session_id,
        next_approval_id
    )
}

fn sorted_sessions(root: &Path) -> Result<Vec<String>> {
    session_ids_by_started_at(root)
}

fn command_label(decision: ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::Approved => "approve",
        ApprovalDecision::Denied => "deny",
    }
}

fn risk_label(risk: RiskClass) -> &'static str {
    match risk {
        RiskClass::Read => "read",
        RiskClass::Write => "write",
        RiskClass::Command => "command",
        RiskClass::Network => "network",
        RiskClass::Delete => "delete",
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs};

    use gorsee_code_agent::{AgentRunError, ChatClient, TaskRunner};
    use gorsee_code_core::{default_agent_matrix, AgentProfile, AgentRole, TaskSpec};
    use gorsee_code_neurogate::{ChatRequest, ChatResponse};
    use serde_json::json;

    use super::*;

    #[test]
    fn approve_resumes_interrupted_execution_with_shared_runner() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"sample\"\n",
        )
        .unwrap();
        let client = MockClient::new(vec![
            json!({
                "message": "write approved change",
                "tool_calls": [{
                    "name": "apply_patch",
                    "args": {
                        "path": "src/lib.rs",
                        "content": "pub fn ready() -> bool { true }\n"
                    }
                }]
            })
            .to_string(),
            final_answer("coder continued"),
        ]);
        let runner = TaskRunner::new(paths::local_dir(temp.path()));
        let spec = TaskSpec::new("ship approved change", temp.path().display().to_string());

        let error = runner
            .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Coder)])
            .unwrap_err();
        let AgentRunError::WaitingApproval(approval_id) = error else {
            panic!("expected waiting approval, got {error:?}");
        };

        let output = decide_with_client(
            temp.path(),
            &approval_id,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap();

        assert!(output.contains(&format!("approve: {approval_id}")));
        assert!(output.contains("status: ready"));
        assert!(output.contains("turn: finished"));
        assert!(output.contains("artifacts: 10"));
        assert_eq!(
            fs::read_to_string(temp.path().join("src/lib.rs")).unwrap(),
            "pub fn ready() -> bool { true }\n"
        );
    }

    fn agent_by_role(role: AgentRole) -> AgentProfile {
        default_agent_matrix()
            .into_iter()
            .find(|agent| agent.role == role)
            .unwrap()
    }

    fn final_answer(answer: &str) -> String {
        json!({
            "message": answer,
            "final_answer": answer
        })
        .to_string()
    }

    struct MockClient {
        replies: RefCell<Vec<String>>,
    }

    impl MockClient {
        fn new(replies: Vec<String>) -> Self {
            Self {
                replies: RefCell::new(replies.into_iter().rev().collect()),
            }
        }
    }

    impl ChatClient for MockClient {
        fn complete(
            &self,
            _request: &ChatRequest,
        ) -> Result<ChatResponse, gorsee_code_agent::AgentRunError> {
            let content = self.replies.borrow_mut().pop().unwrap();
            Ok(ChatResponse {
                id: Some("mock".into()),
                object: Some("chat.completion".into()),
                choices: Some(vec![json!({ "message": { "content": content } })]),
                usage: None,
            })
        }
    }
}
