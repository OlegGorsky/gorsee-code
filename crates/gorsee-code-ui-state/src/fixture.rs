use gorsee_code_core::{default_agent_matrix, AgentStatus, Event, EventKind};
use gorsee_code_usage::{BudgetPolicy, TokenLedger, UsageRecord};
use serde_json::json;

use crate::{AgentView, BudgetView, EventView, MissionControlState, SessionView, ToolCallView};

pub fn fixture_state(name: &str) -> MissionControlState {
    match name {
        "approval" | "approval-waiting" => approval_waiting(),
        "stale-limits" => stale_limits(),
        "failed-tool" => failed_tool(),
        _ => mission_running(),
    }
}

pub fn mission_running() -> MissionControlState {
    state(
        "mission-running",
        "running",
        vec![],
        ledger(57_000),
        "online",
    )
}

pub fn approval_waiting() -> MissionControlState {
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

pub fn stale_limits() -> MissionControlState {
    state(
        "stale-limits",
        "running",
        vec![],
        ledger(42_000),
        "limits_stale",
    )
}

pub fn failed_tool() -> MissionControlState {
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
) -> MissionControlState {
    let agents = default_agent_matrix()
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            AgentView::from_profile(profile, status_for(index), (index as u64 + 1) * 4_000)
        })
        .collect();
    MissionControlState {
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
        title: "Foundation vertical slice".into(),
        status: status.into(),
        repo: ".".into(),
        branch: "main".into(),
    }
}

fn timeline(session_id: &str) -> Vec<EventView> {
    [
        (1, EventKind::MissionStarted, "mission started", None),
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
        phase: "fixture".into(),
        model: "neurogate/gpt-5".into(),
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
    fn fixture_has_agents_and_timeline() {
        let state = mission_running();
        assert_eq!(state.agents.len(), 5);
        assert!(!state.timeline.is_empty());
    }
}
