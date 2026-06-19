use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SessionStarted,
    SessionPaused,
    SessionResumed,
    SessionFinished,
    AgentStarted,
    AgentThinking,
    AgentMessage,
    AgentDelegated,
    ToolRequested,
    ToolApproved,
    ToolDenied,
    ToolStarted,
    ToolFinished,
    ModelCapabilityDetected,
    SkillStarted,
    SkillFinished,
    HookStarted,
    HookFinished,
    SearchStarted,
    SearchFinished,
    ArtifactCreated,
    VisionAnalyzed,
    ImageGenerated,
    PatchProposed,
    PatchApplied,
    TestStarted,
    TestFinished,
    BudgetWarning,
    BudgetExceeded,
    ContextUpdated,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub sequence: u64,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
    pub payload: Value,
}

impl Event {
    pub fn new(
        sequence: u64,
        session_id: impl Into<String>,
        agent_id: Option<String>,
        kind: EventKind,
        payload: Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence,
            session_id: session_id.into(),
            agent_id,
            timestamp: Utc::now(),
            kind,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_serializes_kind_as_snake_case() {
        let event = Event::new(7, "s", None, EventKind::SessionStarted, json!({}));
        let encoded = serde_json::to_value(event).unwrap();
        assert_eq!(encoded["kind"], "session_started");
    }
}
