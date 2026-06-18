use gorsee_code_core::{AgentProfile, AgentStatus, Event, EventKind};
use gorsee_code_usage::BudgetStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionView {
    pub id: String,
    pub title: String,
    pub status: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentView {
    pub id: String,
    pub role: String,
    pub model: String,
    pub status: String,
    pub tokens_used: u64,
    pub tokens_limit: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventView {
    pub sequence: u64,
    pub kind: String,
    pub agent_id: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallView {
    pub id: String,
    pub name: String,
    pub status: String,
    pub risk: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetView {
    pub used_tokens: u64,
    pub limit_tokens: u64,
    pub percent_used: f64,
    pub warning: bool,
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionControlState {
    pub session: SessionView,
    pub agents: Vec<AgentView>,
    pub timeline: Vec<EventView>,
    pub budget: BudgetView,
    pub approvals: Vec<ToolCallView>,
    pub gateway_status: String,
}

impl AgentView {
    pub fn from_profile(profile: &AgentProfile, status: AgentStatus, tokens_used: u64) -> Self {
        Self {
            id: profile.id().into(),
            role: profile.id().into(),
            model: profile.model.clone(),
            status: format!("{status:?}").to_lowercase(),
            tokens_used,
            tokens_limit: profile.budget_tokens,
        }
    }
}

impl EventView {
    pub fn from_event(event: &Event) -> Self {
        Self {
            sequence: event.sequence,
            kind: kind_label(&event.kind).into(),
            agent_id: event.agent_id.clone(),
            summary: summarize_event(event),
        }
    }
}

impl From<BudgetStatus> for BudgetView {
    fn from(status: BudgetStatus) -> Self {
        Self {
            used_tokens: status.used_tokens,
            limit_tokens: status.limit_tokens,
            percent_used: status.percent_used,
            warning: status.warning,
            stopped: status.stopped,
        }
    }
}

fn summarize_event(event: &Event) -> String {
    event
        .payload
        .get("message")
        .or_else(|| event.payload.get("text"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| kind_label(&event.kind).to_string())
}

fn kind_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::MissionStarted => "mission_started",
        EventKind::MissionFinished => "mission_finished",
        EventKind::ToolRequested => "tool_requested",
        EventKind::ToolFinished => "tool_finished",
        EventKind::BudgetWarning => "budget_warning",
        EventKind::Error => "error",
        _ => "event",
    }
}
