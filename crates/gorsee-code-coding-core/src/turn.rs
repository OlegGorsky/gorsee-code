use serde::{Deserialize, Serialize};

use crate::intent::{CodingIntent, IntentDecision};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    pub root: String,
    pub branch: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRequest {
    pub workspace: WorkspaceRef,
    pub message: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnPlan {
    pub goal: String,
    pub summary: String,
    pub intent: CodingIntent,
    pub decision: IntentDecisionSnapshot,
    pub steps: Vec<PlanStep>,
    pub files_to_inspect: Vec<String>,
    pub files_to_modify: Vec<String>,
    pub verification: Vec<String>,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentDecisionSnapshot {
    pub intent: CodingIntent,
    pub requires_tools: bool,
    pub requires_write: bool,
    pub requires_approval: bool,
    pub reason: String,
}

impl From<&IntentDecision> for IntentDecisionSnapshot {
    fn from(decision: &IntentDecision) -> Self {
        Self {
            intent: decision.intent,
            requires_tools: decision.requires_tools,
            requires_write: decision.requires_write,
            requires_approval: decision.requires_approval,
            reason: decision.reason.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub kind: PlanStepKind,
    pub description: String,
    pub expected_tools: Vec<String>,
    pub risk: PlanRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepKind {
    Read,
    Search,
    Edit,
    Command,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanRisk {
    Read,
    Write,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStepState {
    Pending,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnResult {
    pub session_id: String,
    pub turn_id: String,
    pub final_answer: Option<String>,
    pub changed_files: Vec<String>,
    pub verification: Vec<String>,
}
