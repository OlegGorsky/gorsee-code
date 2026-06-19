use chrono::{DateTime, Utc};
use gorsee_code_safety::RiskClass;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

impl ApprovalDecision {
    pub fn status(self) -> ApprovalStatus {
        match self {
            Self::Approved => ApprovalStatus::Approved,
            Self::Denied => ApprovalStatus::Denied,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: String,
    pub session_id: String,
    pub sequence: u64,
    pub agent_id: String,
    pub tool_name: String,
    pub args: Value,
    pub risk: RiskClass,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

impl ApprovalRecord {
    pub fn pending(
        session_id: impl Into<String>,
        sequence: u64,
        agent_id: impl Into<String>,
        tool_name: impl Into<String>,
        args: Value,
        risk: RiskClass,
    ) -> Self {
        Self {
            id: format!("appr_{sequence:04}"),
            session_id: session_id.into(),
            sequence,
            agent_id: agent_id.into(),
            tool_name: tool_name.into(),
            args,
            risk,
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
            decided_at: None,
        }
    }

    pub fn decide(&mut self, decision: ApprovalDecision) {
        self.status = decision.status();
        self.decided_at = Some(Utc::now());
    }
}
