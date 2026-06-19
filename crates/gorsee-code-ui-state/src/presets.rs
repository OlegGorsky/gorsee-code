use gorsee_code_core::{default_agent_matrix, AgentStatus, Event, EventKind};
use gorsee_code_usage::{BudgetPolicy, TokenLedger, UsageRecord};
use serde_json::json;

use crate::{AgentView, BudgetView, EventView, SessionView, ToolCallView, WorkspaceState};

pub fn preset_state(name: &str) -> WorkspaceState {
    match name {
        "approval" | "approval-waiting" => approval_waiting(),
        "stale-limits" => stale_limits(),
        "failed-tool" => failed_tool(),
        _ => workspace_running(),
    }
}

pub fn workspace_running() -> WorkspaceState {
    state(
        "workspace-running",
        "running",
        vec![],
        ledger(57_000),
        "online",
    )
}

pub fn approval_waiting() -> WorkspaceState {
    let approval = ToolCallView {
        id: "tool-1".into(),
        name: "apply_patch".into(),
        status: "waiting_approval".into(),
        risk: "write".into(),
    };
    state(
        "approval-waiting",
        "waiting_approval",
        vec![approval],
        ledger(31_000),
        "online",
    )
}

pub fn stale_limits() -> WorkspaceState {
    state(
        "stale-limits",
        "running",
        vec![],
        ledger(42_000),
        "limits_stale",
    )
}

pub fn failed_tool() -> WorkspaceState {
    let mut view = state("failed-tool", "failed", vec![], ledger(18_000), "online");
    view.timeline.push(EventView::from_event(&Event::new(
        4,
        "failed-tool",
        Some("validator".into()),
        EventKind::Error,
        json!({ "message": "run_test failed" }),
    )));
    view
}

fn state(
    id: &str,
    status: &str,
    approvals: Vec<ToolCallView>,
    ledger: TokenLedger,
    gateway_status: &str,
) -> WorkspaceState {
    let agents = default_agent_matrix()
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            AgentView::from_profile(profile, status_for(index), (index as u64 + 1) * 4_000)
        })
        .collect();
    WorkspaceState {
        session: session(id, status),
        agents,
        timeline: timeline(id),
        budget: BudgetView::from(BudgetPolicy::default().evaluate(&ledger)),
        approvals,
        gateway_status: gateway_status.into(),
    }
}

fn session(id: &str, status: &str) -> SessionView {
    SessionView {
        id: id.into(),
        title: "Gorsee Code Workspace".into(),
        status: status.into(),
        repo: ".".into(),
        branch: "main".into(),
    }
}

fn timeline(session_id: &str) -> Vec<EventView> {
    [
        (1, EventKind::SessionStarted, "session started", None),
        (
            2,
            EventKind::AgentStarted,
            "architect planning",
            Some("architect"),
        ),
        (
            3,
            EventKind::ToolFinished,
            "repo_map complete",
            Some("scout"),
        ),
    ]
    .into_iter()
    .map(|(sequence, kind, message, agent)| {
        EventView::from_event(&Event::new(
            sequence,
            session_id,
            agent.map(str::to_string),
            kind,
            json!({ "message": message }),
        ))
    })
    .collect()
}

fn ledger(tokens: u64) -> TokenLedger {
    let mut ledger = TokenLedger::default();
    ledger.push(UsageRecord {
        agent_id: "coder".into(),
        phase: "workspace".into(),
        model: "glm-5.1".into(),
        input_tokens: tokens,
        output_tokens: 0,
        cached_tokens: 0,
        reasoning_tokens: 0,
        estimated: true,
        credit_multiplier: 1.0,
    });
    ledger
}

fn status_for(index: usize) -> AgentStatus {
    match index {
        0 => AgentStatus::Planning,
        1 => AgentStatus::Reading,
        2 => AgentStatus::Patching,
        3 => AgentStatus::Validating,
        _ => AgentStatus::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_has_agents_and_timeline() {
        let state = workspace_running();
        assert_eq!(state.agents.len(), 5);
        assert!(!state.timeline.is_empty());
    }
}
